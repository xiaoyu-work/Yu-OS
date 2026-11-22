// Named model calls for Yu OS Apps.

package yuossdk

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
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
	AudioBase64  string  `json:"audio_base64"`
	SampleRateHz *uint32 `json:"sample_rate_hz,omitempty"`
}

type Transcription struct {
	Text string `json:"text"`
}

type Image struct {
	Format      string `json:"format"`
	ImageBase64 string `json:"image_base64"`
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
	Tools     []string
	Images    []AiChatImage
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
	if opts.Origin == "" || len(opts.Images) != 0 || len(opts.Functions) != 0 || len(opts.ToolTurns) != 0 {
		if len(opts.Tools) != 0 {
			return nil, &AiInvalidArgsError{Msg: "chat: supply App functions in Functions"}
		}
		if opts.AppID != "" && opts.AppID != os.Getenv("YUOS_APP_ID") {
			return nil, &AiDeniedError{Payload: map[string]any{
				"code": "PERMISSION_DENIED", "error": "chat: App identity does not match the session",
			}}
		}
		return ChatRequest(ctx, AiChatRequest{
			Origin: appInput(opts.Origin), Prompt: prompt, System: opts.System, MaxUnits: uint64(opts.MaxUnits),
			Images: opts.Images, Tools: opts.Functions, ToolTurns: opts.ToolTurns,
		})
	}
	return dispatch(ctx, prompt, opts)
}

// ChatRequest calls the configured chat model with optional App functions and prior results.
func ChatRequest(ctx context.Context, request AiChatRequest) (*AiResponse, error) {
	payload, _, err := encodeModelRequest(request, ValidateAiChatRequest)
	if err != nil {
		return nil, err
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
	if err := ValidateAiInference(out.Data); err != nil {
		return nil, &AiUnavailableError{Msg: err.Error()}
	}
	reply := asMap(out.Data)
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

func STT(ctx context.Context, audioBase64, format string, opts ModelOptions) (*ModelResponse[Transcription], error) {
	return runModel[Transcription](ctx, TranscriptionInput{Kind: "stt", AudioBase64: audioBase64, Format: format}, opts)
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
