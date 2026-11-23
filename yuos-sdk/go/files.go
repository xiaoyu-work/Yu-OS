package yuossdk

import (
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"regexp"
	"strings"
	"unicode"
)

// MaxFileBytes is the maximum file size (32 MiB) for brokered file operations.
const MaxFileBytes = 32 * 1024 * 1024

const fileChunkBytes = 512 * 1024

var (
	fileTokenRe  = regexp.MustCompile(`^[0-9a-f]{32}$`)
	fileSHA256Re = regexp.MustCompile(`^sha256:[0-9a-f]{64}$`)
)

// FileSnapshot holds a complete binary snapshot and an opaque revision for this invocation.
type FileSnapshot struct {
	Content  []byte
	Revision string
}

func validateBrokeredFilePath(path string) error {
	if !strings.HasPrefix(path, "/") || len(path) > 4096 {
		return errors.New("brokered file requires an absolute literal file path")
	}
	for _, part := range strings.Split(path[1:], "/") {
		if part == "" || part == "." || part == ".." {
			return errors.New("brokered file requires an absolute literal file path")
		}
	}
	for _, r := range path {
		if unicode.IsControl(r) || strings.ContainsRune("*?[]{}<>|;&$`\\", r) {
			return errors.New("brokered file requires an absolute literal file path")
		}
	}
	return nil
}

func fileSHA256(data []byte) string {
	sum := sha256.Sum256(data)
	return "sha256:" + hex.EncodeToString(sum[:])
}

func callFileBroker(path string, commit bool, fields map[string]any) (map[string]any, error) {
	body := map[string]any{"path": path}
	for k, v := range fields {
		body[k] = v
	}
	raw, err := json.Marshal(body)
	if err != nil {
		return nil, err
	}
	sub := "transfer"
	if commit {
		sub = "commit"
	}
	out, err := yuCallJSON("yu __app_file", []string{"__app_file", sub}, raw)
	if err != nil {
		return nil, err
	}
	m, ok := out.Data.(map[string]any)
	if !ok {
		return nil, errors.New("broker returned non-object file response")
	}
	return m, nil
}

func extractFileToken(result map[string]any, key string) (string, error) {
	token, _ := result[key].(string)
	if !fileTokenRe.MatchString(token) {
		return "", errors.New("broker returned an invalid file transfer token")
	}
	return token, nil
}

func abortFileTransfer(path, token string) {
	_, _ = callFileBroker(path, false, map[string]any{"action": "abort", "id": token})
}

// ReadFile reads an existing file in a brokered-file MCP invocation.
func ReadFile(path string) (*FileSnapshot, error) {
	if err := validateBrokeredFilePath(path); err != nil {
		return nil, err
	}
	metadata, err := callFileBroker(path, false, map[string]any{"action": "read-start"})
	if err != nil {
		return nil, err
	}
	revision, err := extractFileToken(metadata, "revision")
	if err != nil {
		return nil, err
	}
	complete := false
	defer func() {
		if !complete {
			abortFileTransfer(path, revision)
		}
	}()
	sizeNum, ok := asInt64(metadata["size"])
	digest, _ := metadata["sha256"].(string)
	if !ok || sizeNum < 0 || sizeNum > MaxFileBytes || !fileSHA256Re.MatchString(digest) {
		return nil, errors.New("broker returned invalid file snapshot metadata")
	}
	size := int(sizeNum)
	data := make([]byte, 0, size)
	for offset := 0; offset < size; offset += fileChunkBytes {
		part, err := callFileBroker(path, false, map[string]any{
			"action": "read-chunk",
			"id":     revision,
			"offset": offset,
		})
		if err != nil {
			return nil, err
		}
		gotOffset, ok := asInt64(part["offset"])
		encoded, _ := part["content_base64"].(string)
		if !ok || int(gotOffset) != offset || encoded == "" {
			return nil, errors.New("broker returned an invalid file chunk")
		}
		chunk, err := base64.StdEncoding.DecodeString(encoded)
		if err != nil {
			return nil, err
		}
		expected := fileChunkBytes
		if size-offset < expected {
			expected = size - offset
		}
		if len(chunk) != expected {
			return nil, errors.New("broker returned a partial file chunk")
		}
		data = append(data, chunk...)
	}
	if fileSHA256(data) != digest {
		return nil, errors.New("broker returned a file snapshot with the wrong digest")
	}
	complete = true
	return &FileSnapshot{Content: data, Revision: revision}, nil
}

// ReplaceFile replaces the exact existing file read earlier in this invocation.
func ReplaceFile(path, revision string, content []byte) error {
	if err := validateBrokeredFilePath(path); err != nil {
		return err
	}
	if !fileTokenRe.MatchString(revision) {
		return errors.New("brokered file requires its opaque read revision")
	}
	if len(content) > MaxFileBytes {
		return errors.New("replacement must be at most 32 MiB")
	}
	started, err := callFileBroker(path, false, map[string]any{
		"action":   "write-start",
		"revision": revision,
		"size":     len(content),
		"sha256":   fileSHA256(content),
	})
	if err != nil {
		return err
	}
	transfer, err := extractFileToken(started, "id")
	if err != nil {
		return err
	}
	committing := false
	defer func() {
		if !committing {
			abortFileTransfer(path, transfer)
		}
	}()
	for offset := 0; offset < len(content); offset += fileChunkBytes {
		end := offset + fileChunkBytes
		if end > len(content) {
			end = len(content)
		}
		part := content[offset:end]
		accepted, err := callFileBroker(path, false, map[string]any{
			"action":         "write-chunk",
			"id":             transfer,
			"offset":         offset,
			"content_base64": base64.StdEncoding.EncodeToString(part),
		})
		if err != nil {
			return err
		}
		received, ok := asInt64(accepted["offset"])
		if !ok || int(received) != offset+len(part) {
			return errors.New("broker did not acknowledge the full file chunk")
		}
	}
	committing = true
	result, err := callFileBroker(path, true, map[string]any{"id": transfer})
	if err != nil {
		return err
	}
	committed, _ := result["committed"].(bool)
	_, hasChanged := result["changed"].(bool)
	if !committed || !hasChanged {
		return fmt.Errorf("broker returned no conclusive file commit result")
	}
	return nil
}

func asInt64(v any) (int64, bool) {
	switch n := v.(type) {
	case float64:
		return int64(n), true
	case int:
		return int64(n), true
	case int64:
		return n, true
	case json.Number:
		i, err := n.Int64()
		return i, err == nil
	default:
		return 0, false
	}
}
