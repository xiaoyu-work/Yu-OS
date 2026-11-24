package yuossdk

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"os/exec"
	"strings"
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

	cmd := exec.CommandContext(parent, YuBinary(), "--wire=1", "ai", "chat", "--request-stdin", "--stream")
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
	if parent.Err() != nil {
		return nil, parent.Err()
	}
	if scanner.Err() != nil || final == nil {
		return nil, &AiUnavailableError{Msg: "App chat stream ended without a final response"}
	}
	runErr := cmd.Wait()
	waited = true
	if parent.Err() != nil {
		return nil, parent.Err()
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

// devStreamChat reads the dev server's SSE text deltas. Requests with App
// functions return the whole reply so tool calls keep one decoder.
func devStreamChat(ctx context.Context, request AiChatRequest, onChunk func(string) error) (*AiResponse, error) {
	if len(request.Tools) != 0 || len(request.ToolTurns) != 0 {
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
	body, err := devChatBody(request)
	if err != nil {
		return nil, err
	}
	body["stream"] = true
	resp, err := devPost(ctx, "/chat/completions", body)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	if !strings.HasPrefix(resp.Header.Get("Content-Type"), "text/event-stream") {
		return nil, &AiUnavailableError{Msg: "dev AI endpoint did not return a streaming response"}
	}
	scanner := bufio.NewScanner(resp.Body)
	scanner.Buffer(make([]byte, 4096), maxOutput)
	var text strings.Builder
	var model string
	var usage map[string]any
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if !strings.HasPrefix(line, "data:") {
			continue
		}
		data := strings.TrimSpace(strings.TrimPrefix(line, "data:"))
		if data == "[DONE]" {
			return devChatReply(model, text.String(), usage, nil)
		}
		if data == "" {
			continue
		}
		decoder := json.NewDecoder(strings.NewReader(data))
		decoder.UseNumber()
		var chunk map[string]any
		if err := decoder.Decode(&chunk); err != nil {
			return nil, &AiUnavailableError{Msg: "dev AI stream could not be decoded"}
		}
		if name := asString(chunk["model"]); name != "" {
			model = name
		}
		if reported := asMap(chunk["usage"]); reported != nil {
			usage = reported
		}
		choices, _ := chunk["choices"].([]any)
		if len(choices) == 0 {
			continue
		}
		piece := asString(asMap(asMap(choices[0])["delta"])["content"])
		if piece == "" {
			continue
		}
		text.WriteString(piece)
		if onChunk != nil {
			if err := onChunk(piece); err != nil {
				return nil, err
			}
		}
	}
	if ctx.Err() != nil {
		return nil, ctx.Err()
	}
	return nil, &AiUnavailableError{Msg: "dev AI stream ended without a final event"}
}
