// Named model calls for Yu OS Apps.

package yuossdk

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"strconv"
	"strings"
)

// AiUnavailableError reports a service or transport failure.
// Payload is present for a structured error from Yu OS.
type AiUnavailableError struct {
	Msg     string
	Payload map[string]any
}

func (e *AiUnavailableError) Error() string { return e.Msg }

type AiInvalidArgsError struct {
	Msg     string
	Payload map[string]any
}

func (e *AiInvalidArgsError) Error() string { return e.Msg }

// AiDeniedError reports that the App cannot make the call.
type AiDeniedError struct{ Payload map[string]any }

func (e *AiDeniedError) Error() string {
	if m, ok := e.Payload["error"].(string); ok && m != "" {
		return m
	}
	return "AI call denied"
}

// AiBudgetExceededError reports that the available budget could not cover the call.
type AiBudgetExceededError struct{ Payload map[string]any }

func (e *AiBudgetExceededError) Error() string {
	if m, ok := e.Payload["error"].(string); ok && m != "" {
		return m
	}
	return "AI budget exceeded"
}

// AiSafetyViolationError: the safety pipeline refused the request.
type AiSafetyViolationError struct{ Payload map[string]any }

func (e *AiSafetyViolationError) Error() string {
	if m, ok := e.Payload["error"].(string); ok && m != "" {
		return m
	}
	return "AI safety violation"
}

// ProposedToolCall is a proposal for an App function, not authorization to execute it.
type ProposedToolCall struct {
	ID    string `json:"id"`
	Name  string `json:"name"`
	Input any    `json:"input"`
}

type ModelResponse[T any] struct {
	Provider string `json:"provider"`
	Model    string `json:"model"`
	Units    uint64 `json:"units"`
	Output   T      `json:"output"`
}

type ChatOutput struct {
	Text          string             `json:"text"`
	ToolCalls     []ProposedToolCall `json:"tool_calls,omitempty"`
	ProviderState string             `json:"provider_state,omitempty"`
}

type Embedding struct {
	Vector []float64 `json:"vector"`
}

type Speech struct {
	Format       string  `json:"format"`
	Audio        []byte  `json:"audio_base64"`
	SampleRateHz *uint32 `json:"sample_rate_hz,omitempty"`
}

type Transcription struct {
	Text string `json:"text"`
}

type Image struct {
	Format string `json:"format"`
	Image  []byte `json:"image_base64"`
}

type Decision struct {
	Answers map[string]any `json:"answers"`
}

// AiResponse is the parsed reply from the AI gate. It reuses the
// generated AiUsage / AiBudget / AiReview wire types.
type AiResponse struct {
	Text          string
	Model         string
	Provider      string
	Units         uint64     `json:"-"`
	Output        ChatOutput `json:"-"`
	Verb          string
	Usage         AiUsage
	Budget        AiBudget
	Review        AiReview
	ToolCalls     []ProposedToolCall
	ProviderState string
	Raw           map[string]any
}

// ChatOptions are the optional parameters for Chat.
type ChatOptions struct {
	// Origin retains the explicit route used by older Apps. Leave empty for new calls.
	Origin string
	// MaxUnits caps the spend for this call (0 = let the gate decide).
	MaxUnits int
	// System is an optional App instruction, not the system Agent's prompt.
	System string
	// AppID is the app identity; defaults to $YUOS_APP_ID.
	AppID string
	// Deprecated: Catalog names are not accepted. Use ChatRequest for App functions.
	Tools []string
	// Images are PNG, JPEG or WebP bytes; Yu OS detects each format.
	Images    [][]byte
	Functions []AiAppTool
	ToolTurns []AiToolTurn
}

// Chat sends a single-shot chat completion through the kernel's AI gate.
func Chat(prompt string, opts ChatOptions) (*AiResponse, error) {
	return ChatContext(context.Background(), prompt, opts)
}

// ChatContext inherits cancellation and deadlines from ctx. An MCP handler
// can pass its *MCPCall directly.
func ChatContext(ctx context.Context, prompt string, opts ChatOptions) (*AiResponse, error) {
	if strings.TrimSpace(prompt) == "" {
		return nil, &AiInvalidArgsError{Msg: "chat: prompt must be non-empty"}
	}
	if opts.MaxUnits < 0 {
		return nil, &AiInvalidArgsError{Msg: "chat: MaxUnits must be nonnegative"}
	}
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	isDev := devAiURL() != ""
	if opts.Origin == "" || len(opts.Images) != 0 || len(opts.Functions) != 0 || len(opts.ToolTurns) != 0 || isDev {
		if len(opts.Tools) != 0 && (opts.Origin == "" || len(opts.Images) != 0 || len(opts.Functions) != 0 || len(opts.ToolTurns) != 0) {
			return nil, &AiInvalidArgsError{Msg: "chat: supply App functions in Functions"}
		}
		envApp := os.Getenv("YUOS_APP_ID")
		if opts.AppID != "" && (envApp != "" || !isDev) && opts.AppID != envApp {
			return nil, &AiDeniedError{Payload: map[string]any{
				"code": "PERMISSION_DENIED", "error": "chat: App identity does not match the session",
			}}
		}
		return ChatRequest(ctx, AiChatRequest{
			Origin: appInput(opts.Origin), Prompt: prompt, System: opts.System, MaxUnits: uint64(opts.MaxUnits),
			Images: encodeImages(opts.Images), Tools: opts.Functions, ToolTurns: opts.ToolTurns,
		})
	}
	return dispatch(ctx, prompt, opts)
}

func encodeImages(images [][]byte) []string {
	encoded := make([]string, len(images))
	for i, image := range images {
		encoded[i] = base64.StdEncoding.EncodeToString(image)
	}
	return encoded
}

// ChatStream streams text chunks from the configured chat model to onChunk.
func ChatStream(ctx context.Context, prompt string, opts ChatOptions, onChunk func(string) error) (*AiResponse, error) {
	resp, err := ChatContext(ctx, prompt, opts)
	if err != nil {
		return nil, err
	}
	if resp.Output.Text != "" && onChunk != nil {
		if err := onChunk(resp.Output.Text); err != nil {
			return nil, err
		}
	}
	return resp, nil
}

// ChatRequest calls the configured chat model with optional App functions and prior results.
func ChatRequest(ctx context.Context, request AiChatRequest) (*AiResponse, error) {
	payload, _, err := encodeModelRequest(request, ValidateAiChatRequest)
	if err != nil {
		return nil, err
	}
	if devAiURL() != "" {
		return devChat(ctx, request)
	}
	out, err := yuCallJSONContext(ctx, "yu ai chat", []string{"ai", "chat", "--request-stdin"}, payload)
	if denied, ok := err.(*DeniedError); ok {
		return nil, classifyError(denied.Payload)
	}
	if unavailable, ok := err.(*UnavailableError); ok {
		return nil, &AiUnavailableError{Msg: unavailable.Msg}
	}
	if err != nil {
		return nil, err
	}
	return parseResponse(out.Data)
}

// Infer invokes the owner's configured embedding, speech, image, or decision model.
func Infer(ctx context.Context, request AiInferenceRequest) (*AiInference, error) {
	payload, body, err := encodeModelRequest(request, ValidateAiInferenceRequest)
	if err != nil {
		return nil, err
	}
	var replyData any
	if devAiURL() != "" {
		replyData, err = devInfer(ctx, body)
		if err != nil {
			return nil, err
		}
	} else {
		out, err := yuCallJSONContext(ctx, "yu ai infer", []string{"ai", "infer", "--request-stdin"}, payload)
		if err != nil {
			if denied, ok := err.(*DeniedError); ok {
				return nil, classifyError(denied.Payload)
			}
			if unavailable, ok := err.(*UnavailableError); ok {
				return nil, &AiUnavailableError{Msg: unavailable.Msg}
			}
			return nil, err
		}
		replyData = out.Data
	}
	if err := ValidateAiInference(replyData); err != nil {
		return nil, &AiUnavailableError{Msg: err.Error()}
	}
	reply := asMap(replyData)
	if asMap(reply["output"])["kind"] != asMap(body["input"])["kind"] {
		return nil, &AiUnavailableError{Msg: "AI inference reply does not match the request"}
	}
	return &AiInference{
		Provider: asString(reply["provider"]),
		Model:    asString(reply["model"]),
		Units:    asUint32(reply["units"]),
		Output:   reply["output"],
	}, nil
}

func encodeModelRequest(request any, validate func(any) error) ([]byte, map[string]any, error) {
	payload, err := json.Marshal(request)
	if err != nil {
		return nil, nil, &AiInvalidArgsError{Msg: err.Error()}
	}
	if len(payload) > 720*1024 {
		return nil, nil, &AiInvalidArgsError{Msg: "AI request exceeds the byte limit"}
	}
	decoder := json.NewDecoder(bytes.NewReader(payload))
	decoder.UseNumber()
	var body map[string]any
	if err := decoder.Decode(&body); err != nil {
		return nil, nil, &AiInvalidArgsError{Msg: err.Error()}
	}
	if err := validate(body); err != nil {
		return nil, nil, &AiInvalidArgsError{Msg: err.Error()}
	}
	return payload, body, nil
}

func appInput(origin string) string {
	if origin == "" {
		return "app"
	}
	return origin
}

type ModelOptions struct {
	Origin   string
	MaxUnits uint32
}

func runModel[T any](ctx context.Context, input any, opts ModelOptions) (*ModelResponse[T], error) {
	reply, err := Infer(ctx, AiInferenceRequest{Origin: appInput(opts.Origin), MaxUnits: opts.MaxUnits, Input: input})
	if err != nil {
		return nil, err
	}
	output := asMap(reply.Output)
	if sample, present := output["sample_rate_hz"]; present {
		output["sample_rate_hz"] = asUint32(sample)
	}
	data, err := json.Marshal(output)
	if err != nil {
		return nil, &AiUnavailableError{Msg: err.Error()}
	}
	result := ModelResponse[T]{Provider: reply.Provider, Model: reply.Model, Units: uint64(reply.Units)}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	if err := decoder.Decode(&result.Output); err != nil {
		return nil, &AiUnavailableError{Msg: err.Error()}
	}
	return &result, nil
}

func Embed(ctx context.Context, text string, opts ModelOptions) (*ModelResponse[Embedding], error) {
	return runModel[Embedding](ctx, EmbedInput{Kind: "embed", Text: text}, opts)
}

func TTS(ctx context.Context, text string, opts ModelOptions) (*ModelResponse[Speech], error) {
	return runModel[Speech](ctx, SpeechInput{Kind: "tts", Text: text}, opts)
}

// STT transcribes WAV, MP3 or Ogg audio bytes; Yu OS detects the format.
func STT(ctx context.Context, audio []byte, opts ModelOptions) (*ModelResponse[Transcription], error) {
	input := TranscriptionInput{Kind: "stt", AudioBase64: base64.StdEncoding.EncodeToString(audio)}
	return runModel[Transcription](ctx, input, opts)
}

func ImageGen(ctx context.Context, prompt string, opts ModelOptions) (*ModelResponse[Image], error) {
	return runModel[Image](ctx, ImageInput{Kind: "imagegen", Prompt: prompt}, opts)
}

func Decide(ctx context.Context, state any, questions map[string]any, opts ModelOptions) (*ModelResponse[Decision], error) {
	return runModel[Decision](ctx, DecisionInput{Kind: "decision", State: state, Questions: questions}, opts)
}

// OpenAiPermissionSettings opens Applications settings after a user action.
// It does not authorize AI or retry the failed call.
func OpenAiPermissionSettings(ctx context.Context) error {
	_, err := yuCallJSONContext(ctx, "yu ai settings", []string{"ai", "settings"})
	if denied, ok := err.(*DeniedError); ok {
		return classifyError(denied.Payload)
	}
	return err
}

// Budget returns current-period usage for an app; the CLI does not report
// a cap. appID defaults to $YUOS_APP_ID when empty.
func Budget(appID string) (*BudgetShow, error) {
	return BudgetContext(context.Background(), appID)
}

// BudgetContext is Budget with caller cancellation and deadlines.
func BudgetContext(ctx context.Context, appID string) (*BudgetShow, error) {
	app := appID
	if app == "" {
		app = os.Getenv("YUOS_APP_ID")
	}
	if app == "" {
		return nil, &AiUnavailableError{Msg: "Budget: app_id is required"}
	}
	out, err := yuCallJSONContext(ctx, "yu agent budget show", []string{"agent", "budget", "show", app})
	if err != nil {
		if denied, ok := err.(*DeniedError); ok {
			return nil, &AiUnavailableError{Msg: denied.Error()}
		}
		if ue, ok := err.(*UnavailableError); ok {
			return nil, &AiUnavailableError{Msg: ue.Msg}
		}
		return nil, err
	}
	if err := ValidateBudgetShow(out.Data); err != nil {
		return nil, &AiUnavailableError{Msg: "budget response decode failed: " + err.Error()}
	}
	env := asMap(out.Data)
	return &BudgetShow{
		App:       asString(env["app"]),
		Period:    asString(env["period"]),
		UnitsUsed: asUint64(env["units_used"]),
	}, nil
}

func resolveApp(modality, appID string) (string, error) {
	app := appID
	if app == "" {
		app = os.Getenv("YUOS_APP_ID")
	}
	if app == "" && devAiURL() != "" {
		app = "dev"
	}
	if app == "" {
		return "", &AiDeniedError{Payload: map[string]any{
			"code": "PERMISSION_DENIED", "error": modality + ": App identity is required",
		}}
	}
	return app, nil
}

func writePrivateInput(name, value string) (string, func(), error) {
	file, err := os.CreateTemp("", "yuos-ai-"+name+"-*")
	if err != nil {
		return "", nil, err
	}
	path := file.Name()
	cleanup := func() { _ = os.Remove(path) }
	if err := file.Chmod(0o600); err != nil {
		_ = file.Close()
		cleanup()
		return "", nil, err
	}
	if _, err := file.WriteString(value); err != nil {
		_ = file.Close()
		cleanup()
		return "", nil, err
	}
	if err := file.Sync(); err != nil {
		_ = file.Close()
		cleanup()
		return "", nil, err
	}
	if err := file.Close(); err != nil {
		cleanup()
		return "", nil, err
	}
	return path, cleanup, nil
}

func dispatch(ctx context.Context, prompt string, opts ChatOptions) (*AiResponse, error) {
	app, err := resolveApp("chat", opts.AppID)
	if err != nil {
		return nil, err
	}

	path, cleanup, err := writePrivateInput("prompt", prompt)
	if err != nil {
		return nil, &AiUnavailableError{Msg: fmt.Sprintf("write private prompt: %v", err)}
	}
	defer cleanup()
	argv := []string{"ai", "chat", "--app", app, "--origin", opts.Origin, "--prompt-file", path}
	if opts.MaxUnits != 0 {
		argv = append(argv, "--max-units", strconv.Itoa(opts.MaxUnits))
	}
	if opts.System != "" {
		path, cleanup, err := writePrivateInput("system", opts.System)
		if err != nil {
			return nil, &AiUnavailableError{Msg: fmt.Sprintf("write private system prompt: %v", err)}
		}
		defer cleanup()
		argv = append(argv, "--system-file", path)
	}
	if len(opts.Tools) > 0 {
		argv = append(argv, "--tools", strings.Join(opts.Tools, ","))
	}

	out, err := yuCallJSONContext(ctx, "yu ai chat", argv)
	if err != nil {
		if denied, ok := err.(*DeniedError); ok {
			return nil, classifyError(denied.Payload)
		}
		if ue, ok := err.(*UnavailableError); ok {
			return nil, &AiUnavailableError{Msg: ue.Msg}
		}
		return nil, err
	}
	response, err := parseResponse(out.Data)
	if err != nil {
		return nil, err
	}
	return response, nil
}

func classifyError(env map[string]any) error {
	code := strings.ToUpper(asString(env["code"]))
	switch code {
	case "BUDGET_EXCEEDED":
		return &AiBudgetExceededError{Payload: env}
	case "SAFETY_VIOLATION":
		return &AiSafetyViolationError{Payload: env}
	case "KERNEL_UNAVAILABLE":
		return &AiUnavailableError{Msg: asString(env["error"]), Payload: env}
	case "INVALID_ARGS":
		return &AiInvalidArgsError{Msg: asString(env["error"]), Payload: env}
	case "PERMISSION_DENIED", "UNKNOWN_APP", "UNKNOWN_VERB", "DENIED":
		return &AiDeniedError{Payload: env}
	default:
		return &DeniedError{Payload: env}
	}
}

func parseBudget(blk map[string]any) AiBudget {
	return AiBudget{
		Period:    asString(blk["period"]),
		UnitsUsed: asUint64(blk["units_used"]),
		UnitsCap:  asUint64(blk["units_cap"]),
	}
}

func parseResponse(value any) (*AiResponse, error) {
	if err := ValidateAi(value); err != nil {
		return nil, &AiUnavailableError{Msg: "ai response decode failed: " + err.Error()}
	}
	env := asMap(value)
	usage := asMap(env["usage"])
	review := asMap(env["review"])

	var calls []ProposedToolCall
	if raw, present := env["tool_calls"]; present {
		rawCalls := raw.([]any)
		calls = make([]ProposedToolCall, 0, len(rawCalls))
		for _, c := range rawCalls {
			cm := c.(map[string]any)
			calls = append(calls, ProposedToolCall{
				ID:    asString(cm["id"]),
				Name:  asString(cm["name"]),
				Input: cm["input"],
			})
		}
	}

	text := asString(env["text"])
	units := asUint64(usage["units"])
	providerState := asString(env["provider_state"])
	return &AiResponse{
		Text:     text,
		Model:    asString(env["model"]),
		Provider: asString(env["provider"]),
		Units:    units,
		Output:   ChatOutput{Text: text, ToolCalls: calls, ProviderState: providerState},
		Verb:     asString(env["verb"]),
		Usage: AiUsage{
			InputTokens:  asUint32(usage["input_tokens"]),
			OutputTokens: asUint32(usage["output_tokens"]),
			Units:        units,
		},
		Budget: parseBudget(asMap(env["budget"])),
		Review: AiReview{
			Safety:         asString(review["safety"]),
			PromptRedacted: asBool(review["prompt_redacted"]),
		},
		ToolCalls:     calls,
		ProviderState: providerState,
		Raw:           env,
	}, nil
}

func devAiURL() string {
	if strings.TrimSpace(os.Getenv("YUOS_SESSION")) != "" {
		return ""
	}
	return strings.TrimRight(strings.TrimSpace(os.Getenv("YUOS_DEV_AI_URL")), "/")
}

func devAiModel(kind ...string) string {
	k := "chat"
	if len(kind) > 0 && kind[0] != "" {
		k = kind[0]
	}
	envVar := "YUOS_DEV_AI_CHAT_MODEL"
	defaultModel := "llama3.2"
	switch k {
	case "embed":
		envVar, defaultModel = "YUOS_DEV_AI_EMBED_MODEL", "nomic-embed-text"
	case "tts":
		envVar, defaultModel = "YUOS_DEV_AI_TTS_MODEL", "tts-1"
	case "stt":
		envVar, defaultModel = "YUOS_DEV_AI_STT_MODEL", "whisper-1"
	case "imagegen":
		envVar, defaultModel = "YUOS_DEV_AI_IMAGEGEN_MODEL", "dall-e-3"
	case "decision", "decide":
		envVar, defaultModel = "YUOS_DEV_AI_DECIDE_MODEL", "llama3.2"
	}
	if m := strings.TrimSpace(os.Getenv(envVar)); m != "" {
		return m
	}
	if m := strings.TrimSpace(os.Getenv("YUOS_DEV_AI_MODEL")); m != "" {
		return m
	}
	return defaultModel
}

func devAiKey() string {
	return strings.TrimSpace(os.Getenv("YUOS_DEV_AI_KEY"))
}

func devPostBytes(ctx context.Context, path string, payload map[string]any) ([]byte, error) {
	baseURL := devAiURL()
	if baseURL == "" {
		return nil, &AiUnavailableError{Msg: "local dev AI URL is not configured"}
	}
	body, err := json.Marshal(payload)
	if err != nil {
		return nil, &AiUnavailableError{Msg: err.Error()}
	}
	reqCtx, cancel := context.WithTimeout(ctx, DefaultTimeout)
	defer cancel()
	req, err := http.NewRequestWithContext(reqCtx, http.MethodPost, baseURL+path, bytes.NewReader(body))
	if err != nil {
		return nil, &AiUnavailableError{Msg: err.Error()}
	}
	req.Header.Set("Content-Type", "application/json")
	if key := devAiKey(); key != "" {
		req.Header.Set("Authorization", "Bearer "+key)
	}
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		if ctx.Err() != nil {
			return nil, ctx.Err()
		}
		return nil, &AiUnavailableError{Msg: fmt.Sprintf("dev AI request failed: %v", err)}
	}
	defer resp.Body.Close()
	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return nil, &AiUnavailableError{Msg: fmt.Sprintf("dev AI HTTP %d", resp.StatusCode)}
	}
	raw, err := io.ReadAll(resp.Body)
	if err != nil {
		if ctx.Err() != nil {
			return nil, ctx.Err()
		}
		return nil, &AiUnavailableError{Msg: fmt.Sprintf("dev AI read failed: %v", err)}
	}
	return raw, nil
}

func devPostJSON(ctx context.Context, path string, payload map[string]any) (map[string]any, error) {
	raw, err := devPostBytes(ctx, path, payload)
	if err != nil {
		return nil, err
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	var parsed map[string]any
	if err := decoder.Decode(&parsed); err != nil || parsed == nil {
		return nil, &AiUnavailableError{Msg: fmt.Sprintf("dev AI returned invalid JSON: %v", err)}
	}
	return parsed, nil
}

func devChat(ctx context.Context, request AiChatRequest) (*AiResponse, error) {
	model := devAiModel()
	var messages []map[string]any
	if request.System != "" {
		messages = append(messages, map[string]any{"role": "system", "content": request.System})
	}
	if len(request.Images) > 0 {
		parts := []map[string]any{{"type": "text", "text": request.Prompt}}
		for _, b64 := range request.Images {
			parts = append(parts, map[string]any{
				"type":      "image_url",
				"image_url": map[string]any{"url": "data:image/png;base64," + b64},
			})
		}
		messages = append(messages, map[string]any{"role": "user", "content": parts})
	} else {
		messages = append(messages, map[string]any{"role": "user", "content": request.Prompt})
	}
	for _, turn := range request.ToolTurns {
		toolCallsOut := make([]map[string]any, 0, len(turn.Calls))
		for _, c := range turn.Calls {
			argBytes, err := json.Marshal(c.Input)
			if err != nil {
				return nil, &AiInvalidArgsError{Msg: err.Error()}
			}
			toolCallsOut = append(toolCallsOut, map[string]any{
				"id":       c.Id,
				"type":     "function",
				"function": map[string]any{"name": c.Name, "arguments": string(argBytes)},
			})
		}
		assistantMsg := map[string]any{"role": "assistant", "tool_calls": toolCallsOut}
		if turn.Text != "" {
			assistantMsg["content"] = turn.Text
		}
		messages = append(messages, assistantMsg)
		for _, r := range turn.Results {
			messages = append(messages, map[string]any{
				"role":         "tool",
				"tool_call_id": r.Id,
				"content":      r.Content,
			})
		}
	}
	body := map[string]any{"model": model, "messages": messages}
	if len(request.Tools) > 0 {
		toolsOut := make([]map[string]any, 0, len(request.Tools))
		for _, t := range request.Tools {
			toolsOut = append(toolsOut, map[string]any{
				"type": "function",
				"function": map[string]any{
					"name":        t.Name,
					"description": t.Description,
					"parameters":  t.Parameters,
				},
			})
		}
		body["tools"] = toolsOut
	}
	if request.MaxUnits > 0 {
		body["max_tokens"] = request.MaxUnits
	}
	resp, err := devPostJSON(ctx, "/chat/completions", body)
	if err != nil {
		return nil, err
	}
	var msg map[string]any
	if choices, ok := resp["choices"].([]any); ok && len(choices) > 0 {
		msg = asMap(asMap(choices[0])["message"])
	}
	text := asString(msg["content"])
	var parsedToolCalls []any
	if rawCalls, ok := msg["tool_calls"].([]any); ok {
		for i, item := range rawCalls {
			tc := asMap(item)
			fn := asMap(tc["function"])
			name := asString(fn["name"])
			if name == "" {
				return nil, &AiUnavailableError{Msg: "dev AI tool_call missing function name"}
			}
			var decodedArgs any = map[string]any{}
			switch rawArgs := fn["arguments"].(type) {
			case string:
				if strings.TrimSpace(rawArgs) != "" {
					dec := json.NewDecoder(strings.NewReader(rawArgs))
					dec.UseNumber()
					if err := dec.Decode(&decodedArgs); err != nil {
						return nil, &AiUnavailableError{Msg: fmt.Sprintf("dev AI tool_call arguments invalid JSON: %v", err)}
					}
				}
			case nil:
			default:
				decodedArgs = rawArgs
			}
			id := asString(tc["id"])
			if id == "" {
				id = fmt.Sprintf("call_%d", i)
			}
			parsedToolCalls = append(parsedToolCalls, map[string]any{
				"id":    id,
				"name":  name,
				"input": decodedArgs,
			})
		}
	}
	usageObj := asMap(resp["usage"])
	units := uint64(1)
	if _, present := usageObj["total_tokens"]; present {
		units = asUint64(usageObj["total_tokens"])
	}
	promptTokens := asUint32(usageObj["prompt_tokens"])
	completionTokens := asUint32(usageObj["completion_tokens"])
	actualModel := asString(resp["model"])
	if strings.TrimSpace(actualModel) == "" {
		actualModel = model
	}
	env := map[string]any{
		"text":     text,
		"model":    actualModel,
		"provider": "dev",
		"verb":     "ai.chat",
		"usage": map[string]any{
			"input_tokens":  json.Number(strconv.FormatUint(uint64(promptTokens), 10)),
			"output_tokens": json.Number(strconv.FormatUint(uint64(completionTokens), 10)),
			"units":         json.Number(strconv.FormatUint(units, 10)),
		},
		"budget": map[string]any{
			"period":     "dev",
			"units_used": json.Number("0"),
			"units_cap":  json.Number("0"),
		},
		"review": map[string]any{
			"safety":          "strict",
			"prompt_redacted": false,
		},
	}
	if len(parsedToolCalls) > 0 {
		env["tool_calls"] = parsedToolCalls
	}
	return parseResponse(env)
}

func devInfer(ctx context.Context, body map[string]any) (map[string]any, error) {
	inp := asMap(body["input"])
	kind := asString(inp["kind"])
	model := devAiModel(kind)
	units := uint32(1)
	actualModel := model
	var output map[string]any
	switch kind {
	case "embed":
		resp, err := devPostJSON(ctx, "/embeddings", map[string]any{
			"model": model,
			"input": asString(inp["text"]),
		})
		if err != nil {
			return nil, err
		}
		data, ok := resp["data"].([]any)
		if !ok || len(data) == 0 {
			return nil, &AiUnavailableError{Msg: "dev AI embeddings response missing data[0]"}
		}
		embedding, ok := asMap(data[0])["embedding"].([]any)
		if !ok {
			return nil, &AiUnavailableError{Msg: "dev AI embeddings response missing embedding vector"}
		}
		output = map[string]any{"kind": "embed", "vector": embedding}
		if m := strings.TrimSpace(asString(resp["model"])); m != "" {
			actualModel = m
		}
		if u := asUint32(asMap(resp["usage"])["total_tokens"]); u >= 1 {
			units = u
		}
	case "tts":
		raw, err := devPostBytes(ctx, "/audio/speech", map[string]any{
			"model":           model,
			"input":           asString(inp["text"]),
			"voice":           "alloy",
			"response_format": "wav",
		})
		if err != nil {
			return nil, err
		}
		var audioB64 string
		format := "wav"
		var parsed map[string]any
		if err := json.Unmarshal(raw, &parsed); err == nil && parsed != nil {
			if s, ok := parsed["audio_base64"].(string); ok {
				audioB64 = s
				if f, ok := parsed["format"].(string); ok && f != "" {
					format = f
				}
			}
		}
		if audioB64 == "" {
			if len(raw) == 0 {
				return nil, &AiUnavailableError{Msg: "dev AI speech response is empty"}
			}
			audioB64 = base64.StdEncoding.EncodeToString(raw)
		}
		output = map[string]any{"kind": "tts", "format": format, "audio_base64": audioB64}
	case "stt":
		resp, err := devPostJSON(ctx, "/audio/transcriptions", map[string]any{
			"model":        model,
			"audio_base64": asString(inp["audio_base64"]),
		})
		if err != nil {
			return nil, err
		}
		text, ok := resp["text"].(string)
		if !ok {
			return nil, &AiUnavailableError{Msg: "dev AI transcription response missing text"}
		}
		output = map[string]any{"kind": "stt", "text": text}
	case "imagegen":
		resp, err := devPostJSON(ctx, "/images/generations", map[string]any{
			"model":           model,
			"prompt":          asString(inp["prompt"]),
			"response_format": "b64_json",
		})
		if err != nil {
			return nil, err
		}
		data, ok := resp["data"].([]any)
		if !ok || len(data) == 0 {
			return nil, &AiUnavailableError{Msg: "dev AI imagegen response missing data[0]"}
		}
		b64JSON, ok := asMap(data[0])["b64_json"].(string)
		if !ok {
			return nil, &AiUnavailableError{Msg: "dev AI imagegen response missing b64_json"}
		}
		output = map[string]any{"kind": "imagegen", "format": "png", "image_base64": b64JSON}
	case "decision", "decide":
		userPayload, err := json.Marshal(map[string]any{
			"state":     inp["state"],
			"questions": inp["questions"],
		})
		if err != nil {
			return nil, &AiInvalidArgsError{Msg: err.Error()}
		}
		resp, err := devPostJSON(ctx, "/chat/completions", map[string]any{
			"model":           model,
			"response_format": map[string]any{"type": "json_object"},
			"messages": []map[string]any{
				{"role": "system", "content": `Return JSON {"answers": {...}} answering each question.`},
				{"role": "user", "content": string(userPayload)},
			},
		})
		if err != nil {
			return nil, err
		}
		choices, ok := resp["choices"].([]any)
		if !ok || len(choices) == 0 {
			return nil, &AiUnavailableError{Msg: "dev AI decision response missing choices[0]"}
		}
		content, ok := asMap(asMap(choices[0])["message"])["content"].(string)
		if !ok {
			return nil, &AiUnavailableError{Msg: "dev AI decision response missing message content"}
		}
		dec := json.NewDecoder(strings.NewReader(content))
		dec.UseNumber()
		var parsedContent map[string]any
		if err := dec.Decode(&parsedContent); err != nil || parsedContent == nil {
			return nil, &AiUnavailableError{Msg: fmt.Sprintf("dev AI decision content is not valid JSON: %v", err)}
		}
		answers, ok := parsedContent["answers"].(map[string]any)
		if !ok {
			return nil, &AiUnavailableError{Msg: "dev AI decision content missing answers object"}
		}
		output = map[string]any{"kind": kind, "answers": answers}
	default:
		return nil, &AiInvalidArgsError{Msg: "unsupported inference kind: " + kind}
	}
	return map[string]any{
		"provider": "dev",
		"model":    actualModel,
		"units":    json.Number(strconv.FormatUint(uint64(units), 10)),
		"output":   output,
	}, nil
}
