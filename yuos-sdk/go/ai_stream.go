package yuossdk

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"os/exec"
	"time"
	"unicode/utf8"
)

// ChatStream delivers text deltas while the OS model runs and returns the
// final response. A callback error stops the call.
func ChatStream(parent context.Context, prompt string, opts ChatOptions, onChunk func(string) error) (*AiResponse, error) {
	if len(opts.Tools) != 0 {
		return nil, &AiInvalidArgsError{Msg: "chat: supply App functions in Functions"}
	}
	request, err := structuredChatRequest(prompt, opts)
	if err != nil {
		return nil, err
	}
	payload, _, err := encodeModelRequest(request, ValidateAiChatRequest)
	if err != nil {
		return nil, err
	}
	if devAiURL() != "" {
		return devStreamChat(parent, request, onChunk)
	}

	ctx, cancel := context.WithTimeout(parent, DefaultTimeout)
	defer cancel()
	cmd := exec.CommandContext(ctx, YuBinary(), "--wire=1", "ai", "chat", "--request-stdin", "--stream")
	cmd.Stdin = bytes.NewReader(payload)
	cmd.WaitDelay = time.Second
	stdout, err := cmd.StdoutPipe()
	if err == nil {
		err = cmd.Start()
	}
	if err != nil {
		if parent.Err() != nil {
			return nil, parent.Err()
		}
		return nil, &AiUnavailableError{Msg: "cannot start yu ai chat: " + err.Error()}
	}
	waited := false
	defer func() {
		if !waited {
			_ = cmd.Process.Kill()
			_ = cmd.Wait()
		}
	}()

	scanner := bufio.NewScanner(stdout)
	scanner.Buffer(make([]byte, 4096), maxOutput)
	total := 0
	var final any
	for scanner.Scan() {
		if final != nil || !utf8.Valid(scanner.Bytes()) {
			return nil, &AiUnavailableError{Msg: "unexpected data in App chat stream"}
		}
		line := json.NewDecoder(bytes.NewReader(scanner.Bytes()))
		line.UseNumber()
		var event any
		if err := line.Decode(&event); err != nil || line.Decode(new(any)) != io.EOF {
			return nil, &AiUnavailableError{Msg: "invalid JSON in App chat stream"}
		}
		if record := asMap(event); record != nil && record["event"] != nil {
			if err := ValidateAiChatDelta(event); err != nil {
				return nil, &AiUnavailableError{Msg: err.Error()}
			}
			text := asString(record["text"])
			total += len(text)
			if total > 512*1024 {
				return nil, &AiUnavailableError{Msg: "App chat stream exceeded its text limit"}
			}
			if onChunk != nil {
				if err := onChunk(text); err != nil {
					return nil, err
				}
			}
		} else {
			if err := ValidateEnvelope(event); err != nil {
				return nil, &AiUnavailableError{Msg: err.Error()}
			}
			final = event
		}
	}
	if ctx.Err() != nil {
		if parent.Err() != nil {
			return nil, parent.Err()
		}
		return nil, &AiUnavailableError{Msg: "App chat stream timed out"}
	}
	if scanner.Err() != nil || final == nil {
		return nil, &AiUnavailableError{Msg: "App chat stream ended without a final response"}
	}
	runErr := cmd.Wait()
	waited = true
	if parent.Err() != nil {
		return nil, parent.Err()
	}
	if ctx.Err() != nil {
		return nil, &AiUnavailableError{Msg: "App chat stream timed out"}
	}
	status := 0
	if runErr != nil {
		var exit *exec.ExitError
		if !errors.As(runErr, &exit) {
			return nil, &AiUnavailableError{Msg: "App chat stream process failed: " + runErr.Error()}
		}
		status = exit.ExitCode()
	}
	result, err := decodeYuEnvelope(final, status, "yu ai chat")
	if denied, ok := err.(*DeniedError); ok {
		return nil, classifyError(denied.Payload)
	}
	if err != nil {
		return nil, &AiUnavailableError{Msg: err.Error()}
	}
	return parseResponse(result.Data)
}

func devStreamChat(ctx context.Context, request AiChatRequest, onChunk func(string) error) (*AiResponse, error) {
	reply, err := devChat(ctx, request)
	if err != nil {
		return nil, err
	}
	if onChunk != nil && reply.Output.Text != "" {
		if err := onChunk(reply.Output.Text); err != nil {
			return nil, err
		}
	}
	return reply, nil
}
