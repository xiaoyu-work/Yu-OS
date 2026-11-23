package yuossdk

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"reflect"
	"slices"
	"strings"
	"testing"
)

const okChat = `{
  "verb": "ai.chat",
  "text": "hello there",
  "model": "m",
  "provider": "p",
  "usage": {"input_tokens": 3, "output_tokens": 5, "units": 8},
  "budget": {"period": "2026-05", "units_used": 8, "units_cap": 1000},
  "review": {"safety": "strict", "prompt_redacted": false},
  "tool_calls": [{"id": "c1", "name": "fs.read_text", "input": {"path": "/x"}}]
}`

func TestChatBuildsArgvAndParses(t *testing.T) {
	bin, argvOut := fakeYu(t, wireSuccess(okChat), 0)
	var res *AiResponse
	var err error
	withCos(t, bin, map[string]string{"YUOS_APP_ID": "notes"}, func() {
		res, err = Chat("summarise", ChatOptions{
			Origin:   "external-content",
			MaxUnits: 100,
			Tools:    []string{"fs.read_text"},
		})
	})
	if err != nil {
		t.Fatalf("Chat returned error: %v", err)
	}
	if res.Output.Text != "hello there" || res.Output.Text != res.Text {
		t.Fatalf("output text = %q (legacy text %q)", res.Output.Text, res.Text)
	}
	if res.Units != 8 || res.Units != res.Usage.Units {
		t.Fatalf("units = %v (legacy units %v)", res.Units, res.Usage.Units)
	}
	if res.Budget.UnitsCap != 1000 {
		t.Fatalf("units_cap = %v", res.Budget.UnitsCap)
	}
	if len(res.ToolCalls) != 1 || res.ToolCalls[0].Name != "fs.read_text" {
		t.Fatalf("tool_calls = %+v", res.ToolCalls)
	}

	argv := readArgv(t, argvOut)
	want := []string{"--wire=1", "ai", "chat", "--app", "notes", "--origin", "external-content"}
	for i, w := range want {
		if argv[i] != w {
			t.Fatalf("argv[%d] = %q, want %q (full %v)", i, argv[i], w, argv)
		}
	}
	if !slices.Contains(argv, "--prompt-file") || !slices.Contains(argv, "--max-units") || !slices.Contains(argv, "--tools") {
		t.Fatalf("missing expected flags: %v", argv)
	}
}

func TestChatEmptyPrompt(t *testing.T) {
	if _, err := Chat("   ", ChatOptions{AppID: "x", Origin: "trusted"}); err == nil {
		t.Fatal("expected error for empty prompt")
	}
}

// fakeYuCapturingStdin is fakeYu that also records the request on its stdin.
func fakeYuCapturingStdin(t *testing.T, stdout string) (bin, stdinOut string) {
	t.Helper()
	bin, _ = fakeYu(t, stdout, 0)
	stdinOut = bin + ".stdin"
	script := "#!/bin/sh\ncat > " + stdinOut + "\nprintf '%s\\n' '" + stdout + "'\n"
	if err := os.WriteFile(bin, []byte(script), 0o755); err != nil {
		t.Fatal(err)
	}
	return bin, stdinOut
}

func TestModelCallsTakeAndReturnMediaBytes(t *testing.T) {
	png, wav := []byte("\x89PNG\r\n\x1a\nfixture"), []byte("RIFF\x24\x00\x00\x00WAVEfmt ")
	b64 := base64.StdEncoding.EncodeToString
	inference := func(output string) string {
		return wireSuccess(`{"provider":"p","model":"m","units":50,"output":` + output + `}`)
	}
	sent := func(path string) map[string]any {
		data, err := os.ReadFile(path)
		if err != nil {
			t.Fatal(err)
		}
		var request map[string]any
		if err := json.Unmarshal(data, &request); err != nil {
			t.Fatal(err)
		}
		return request
	}
	ctx := context.Background()

	bin, stdin := fakeYuCapturingStdin(t, wireSuccess(okChat))
	withCos(t, bin, nil, func() {
		if _, err := Chat("Describe this image.", ChatOptions{Images: [][]byte{png}}); err != nil {
			t.Fatal(err)
		}
	})
	if images := sent(stdin)["images"]; !reflect.DeepEqual(images, []any{b64(png)}) {
		t.Fatalf("images = %#v", images)
	}
	bin, stdin = fakeYuCapturingStdin(t, inference(`{"kind":"stt","text":"hello"}`))
	withCos(t, bin, nil, func() {
		if reply, err := STT(ctx, wav, ModelOptions{}); err != nil || reply.Output.Text != "hello" {
			t.Fatalf("STT = %+v, %v", reply, err)
		}
	})
	if input := sent(stdin)["input"]; !reflect.DeepEqual(input, map[string]any{"kind": "stt", "audio_base64": b64(wav)}) {
		t.Fatalf("STT input = %#v", input)
	}

	bin, _ = fakeYu(t, inference(`{"kind":"tts","format":"wav","audio_base64":"`+b64(wav)+`"}`), 0)
	withCos(t, bin, nil, func() {
		speech, err := TTS(ctx, "Hello", ModelOptions{})
		if err != nil || speech.Output.Format != "wav" || !bytes.Equal(speech.Output.Audio, wav) {
			t.Fatalf("TTS = %+v, %v", speech, err)
		}
	})
	bin, _ = fakeYu(t, inference(`{"kind":"imagegen","format":"png","image_base64":"`+b64(png)+`"}`), 0)
	withCos(t, bin, nil, func() {
		image, err := ImageGen(ctx, "A cat", ModelOptions{})
		if err != nil || image.Output.Format != "png" || !bytes.Equal(image.Output.Image, png) {
			t.Fatalf("ImageGen = %+v, %v", image, err)
		}
	})
	var invalid *AiInvalidArgsError
	if _, err := STT(ctx, nil, ModelOptions{}); !errors.As(err, &invalid) {
		t.Fatalf("empty audio: want AiInvalidArgsError, got %T %v", err, err)
	}
}

func TestChatRequiresAppID(t *testing.T) {
	bin, _ := fakeYu(t, wireSuccess(okChat), 0)
	var err error
	withCos(t, bin, map[string]string{"YUOS_APP_ID": ""}, func() {
		_, err = Chat("hi", ChatOptions{Origin: "trusted"})
	})
	if _, ok := err.(*AiDeniedError); !ok {
		t.Fatalf("expected AiDeniedError, got %T %v", err, err)
	}
}

func TestChatErrorClassification(t *testing.T) {
	cases := []struct {
		body string
		want string
	}{
		{wireErrorEnvelope("safety words", "BUDGET_EXCEEDED"), "budget"},
		{wireErrorEnvelope("budget words", "SAFETY_VIOLATION"), "safety"},
		{wireErrorEnvelope("capability denied", "PERMISSION_DENIED"), "denied"},
		{wireErrorEnvelope("opaque", "KERNEL_UNAVAILABLE"), "unavailable"},
		{wireErrorEnvelope("opaque", "INVALID_ARGS"), "invalid"},
	}
	for _, c := range cases {
		bin, _ := fakeYu(t, c.body, 1)
		var err error
		withCos(t, bin, map[string]string{"YUOS_APP_ID": "notes"}, func() {
			_, err = Chat("hi", ChatOptions{})
		})
		switch c.want {
		case "budget":
			if _, ok := err.(*AiBudgetExceededError); !ok {
				t.Fatalf("%s: want AiBudgetExceededError, got %T", c.body, err)
			}
		case "safety":
			if _, ok := err.(*AiSafetyViolationError); !ok {
				t.Fatalf("%s: want AiSafetyViolationError, got %T", c.body, err)
			}
		case "unavailable":
			if e, ok := err.(*AiUnavailableError); !ok || e.Payload["code"] != "KERNEL_UNAVAILABLE" {
				t.Fatalf("%s: want server AiUnavailableError, got %T", c.body, err)
			}
		case "invalid":
			if _, ok := err.(*AiInvalidArgsError); !ok {
				t.Fatalf("%s: want AiInvalidArgsError, got %T", c.body, err)
			}
		default:
			if _, ok := err.(*AiDeniedError); !ok {
				t.Fatalf("%s: want AiDeniedError, got %T", c.body, err)
			}
		}
	}
}

func TestChatRejectsMalformedToolCall(t *testing.T) {
	body := `{
	  "verb":"ai.chat","text":"hello","model":"m","provider":"p",
	  "usage":{"input_tokens":1,"output_tokens":1,"units":2},
	  "budget":{"period":"2026-08","units_used":2,"units_cap":100},
	  "review":{"safety":"strict","prompt_redacted":false},
	  "tool_calls":[{"id":"c1","input":{}}]
	}`
	bin, _ := fakeYu(t, wireSuccess(body), 0)
	var err error
	withCos(t, bin, map[string]string{"YUOS_APP_ID": "notes"}, func() {
		_, err = Chat("hi", ChatOptions{Origin: "trusted"})
	})
	unavailable, ok := err.(*AiUnavailableError)
	if !ok || !strings.Contains(unavailable.Msg, "WIRE_REQUIRED") ||
		!strings.Contains(unavailable.Msg, "$.tool_calls[0].name") {
		t.Fatalf("expected strict decode error, got %T %v", err, err)
	}
}

func TestChatAcceptsMathematicalIntegersAndUnrestrictedToolInput(t *testing.T) {
	body := `{
	  "verb":"ai.chat","text":"hello","model":"m","provider":"p",
	  "usage":{"input_tokens":1.0,"output_tokens":1e0,"units":2.0},
	  "budget":{"period":"2026-08","units_used":2e0,"units_cap":100.0},
	  "review":{"safety":"strict","prompt_redacted":false},
	  "tool_calls":[{"id":"c1","name":"echo","input":["a",1]}]
	}`
	bin, _ := fakeYu(t, wireSuccess(body), 0)
	var response *AiResponse
	var err error
	withCos(t, bin, map[string]string{"YUOS_APP_ID": "notes"}, func() {
		response, err = Chat("hi", ChatOptions{Origin: "trusted"})
	})
	if err != nil {
		t.Fatal(err)
	}
	if response.Usage.InputTokens != 1 || response.Usage.OutputTokens != 1 {
		t.Fatalf("usage = %+v", response.Usage)
	}
	if input, ok := response.ToolCalls[0].Input.([]any); !ok || len(input) != 2 {
		t.Fatalf("tool input = %#v", response.ToolCalls[0].Input)
	}
}

func TestChatRejectsScalarRootWithStableWireError(t *testing.T) {
	bin, _ := fakeYu(t, `null`, 0)
	var err error
	withCos(t, bin, map[string]string{"YUOS_APP_ID": "notes"}, func() {
		_, err = Chat("hi", ChatOptions{Origin: "trusted"})
	})
	unavailable, ok := err.(*AiUnavailableError)
	if !ok || !strings.Contains(unavailable.Msg, "WIRE_ONE_OF") ||
		!strings.Contains(unavailable.Msg, " at $:") {
		t.Fatalf("expected stable root decode error, got %T %v", err, err)
	}
}

func TestBudgetAcceptsProducerShape(t *testing.T) {
	bin, _ := fakeYu(t, wireSuccess(`{"app":"notes","period":"2026-08","units_used":7}`), 0)
	var budget *BudgetShow
	var err error
	withCos(t, bin, nil, func() {
		budget, err = Budget("notes")
	})
	if err != nil {
		t.Fatal(err)
	}
	if *budget != (BudgetShow{App: "notes", Period: "2026-08", UnitsUsed: 7}) {
		t.Fatalf("budget = %+v", budget)
	}
}

func TestDevAIModeAndSessionOverride(t *testing.T) {
	var paths []string
	var authHeader string
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		paths = append(paths, r.URL.Path)
		authHeader = r.Header.Get("Authorization")
		_, _ = io.ReadAll(r.Body)
		w.Header().Set("Content-Type", "application/json")
		switch r.URL.Path {
		case "/v1/chat/completions":
			_, _ = w.Write([]byte(`{"model":"llama3.2","choices":[{"message":{"role":"assistant","content":"dev go reply"}}],"usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}}`))
		case "/v1/embeddings":
			_, _ = w.Write([]byte(`{"model":"llama3.2","data":[{"embedding":[0.5,1.5]}],"usage":{"total_tokens":4}}`))
		default:
			w.WriteHeader(http.StatusNotFound)
		}
	}))
	defer server.Close()

	bin, _ := fakeYu(t, wireSuccess(okChat), 0)
	withCos(t, bin, map[string]string{
		"YUOS_DEV_AI_URL": server.URL + "/v1/",
		"YUOS_DEV_AI_KEY": "go-dev-key",
		"YUOS_SESSION":    "",
		"YUOS_APP_ID":     "",
	}, func() {
		chatRes, err := Chat("hello dev", ChatOptions{})
		if err != nil {
			t.Fatalf("Chat in dev mode failed: %v", err)
		}
		if chatRes.Text != "dev go reply" || chatRes.Provider != "dev" || chatRes.Model != "llama3.2" || chatRes.Units != 5 {
			t.Fatalf("unexpected dev Chat response: %+v", chatRes)
		}
		embedRes, err := Embed(context.Background(), "embed this", ModelOptions{})
		if err != nil {
			t.Fatalf("Embed in dev mode failed: %v", err)
		}
		if embedRes.Provider != "dev" || !reflect.DeepEqual(embedRes.Output.Vector, []float64{0.5, 1.5}) {
			t.Fatalf("unexpected dev Embed response: %+v", embedRes)
		}
	})
	if !reflect.DeepEqual(paths, []string{"/v1/chat/completions", "/v1/embeddings"}) {
		t.Fatalf("paths = %v", paths)
	}
	if authHeader != "Bearer go-dev-key" {
		t.Fatalf("authHeader = %q", authHeader)
	}

	// When YUOS_SESSION is set, YUOS_DEV_AI_URL must be ignored and the `yu` binary used.
	paths = nil
	withCos(t, bin, map[string]string{
		"YUOS_DEV_AI_URL": server.URL + "/v1",
		"YUOS_SESSION":    "sandbox-session-1",
		"YUOS_APP_ID":     "notes",
	}, func() {
		res, err := Chat("hello sandbox", ChatOptions{})
		if err != nil {
			t.Fatalf("Chat in sandbox mode failed: %v", err)
		}
		if res.Text != "hello there" || res.Provider != "p" {
			t.Fatalf("expected yu binary response, got %+v", res)
		}
	})
	if len(paths) != 0 {
		t.Fatalf("expected dev server not to be called when YUOS_SESSION is set, got %v", paths)
	}
}
