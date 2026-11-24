// Command-line and desktop functions for a Yu OS App.
//
//	yuossdk.HandleCommand("summarize", summarize) // yu app <id> summarize "text"
//	yuossdk.HandleDesktop(openWindow)             // App launcher or yu app <id> --gui
//	if err := yuossdk.RunApp(); err != nil {
//		log.Fatal(err)
//	}
//
// Declare the command under `operations` and the window under `desktop` in
// app.json.

package yuossdk

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
)

// CommandFunc receives a command's validated arguments and returns a JSON value.
type CommandFunc func(args []string) (any, error)

// DesktopFunc opens the App's desktop window.
type DesktopFunc func() error

var (
	commandFuncs = map[string]CommandFunc{}
	desktopFunc  DesktopFunc
	devClipboard string
)

// HandleCommand runs fn for `yu app <id> <name>`.
func HandleCommand(name string, fn CommandFunc) {
	if name == "" || fn == nil {
		panic("yuossdk: HandleCommand needs a name and function")
	}
	if _, exists := commandFuncs[name]; exists {
		panic(fmt.Sprintf("yuossdk: command %q already has a function", name))
	}
	commandFuncs[name] = fn
}

// DataDir returns the App's private data directory, creating it if needed.
func DataDir() (string, error) {
	dir := os.Getenv("YUOS_DATA_DIR")
	if dir == "" {
		base := os.Getenv("XDG_DATA_HOME")
		if base == "" {
			home, err := os.UserHomeDir()
			if err != nil {
				return "", err
			}
			base = filepath.Join(home, ".local", "share")
		}
		appID := os.Getenv("YUOS_APP_ID")
		if appID == "" {
			appID = "dev"
		}
		dir = filepath.Join(base, "yuos-dev", "apps", appID)
	}
	if err := os.MkdirAll(dir, 0o700); err != nil {
		return "", err
	}
	return dir, nil
}

// Notify posts a system notification from this App (`ui.notify`).
func Notify(summary, body string) error {
	if strings.TrimSpace(summary) == "" {
		return errors.New("notify: summary must be non-empty")
	}
	if os.Getenv("YUOS_SESSION") == "" {
		return nil
	}
	payload, err := json.Marshal(map[string]any{
		"action":    "post",
		"summary":   summary,
		"body":      body,
		"app_name":  os.Getenv("YUOS_APP_ID"),
		"icon":      "",
		"expire_ms": 0,
		"transient": false,
	})
	if err != nil {
		return err
	}
	_, err = yuCallJSON("yu __notifications", []string{"__notifications", "request", "--request-stdin", "--deadline", "9999999999999"}, payload)
	return err
}

// ClipboardRead reads text from the system clipboard (`clipboard.read`).
func ClipboardRead() (string, error) {
	if os.Getenv("YUOS_SESSION") == "" {
		if v, ok := os.LookupEnv("YUOS_DEV_CLIPBOARD"); ok {
			return v, nil
		}
		return devClipboard, nil
	}
	out, err := yuCallJSON("yu __clipboard", []string{"__clipboard", "read"}, nil)
	if err != nil {
		return "", err
	}
	if m, ok := out.Data.(map[string]any); ok {
		if t, ok := m["text"].(string); ok {
			return t, nil
		}
	}
	return "", nil
}

// ClipboardWrite writes text to the system clipboard (`clipboard.write`).
func ClipboardWrite(text string) error {
	if os.Getenv("YUOS_SESSION") == "" {
		devClipboard = text
		return nil
	}
	dir, err := DataDir()
	if err != nil {
		return err
	}
	scratch := filepath.Join(dir, ".clipboard-tmp")
	if err := os.WriteFile(scratch, []byte(text), 0o600); err != nil {
		return err
	}
	defer os.Remove(scratch)
	_, err = yuCallJSON("yu __clipboard", []string{"__clipboard", "write", "--source", scratch, "--confirm"}, nil)
	return err
}

// Secret loads a secret from the Yu OS credential store (`secret.read`).
func Secret(name, namespace string) (string, error) {
	name = strings.TrimSpace(name)
	if name == "" {
		return "", errors.New("secret: name must be non-empty")
	}
	namespace = strings.TrimSpace(namespace)
	if namespace == "" {
		namespace = "default"
	}
	var b strings.Builder
	b.WriteString("YUOS_DEV_SECRET_")
	for _, r := range strings.ToUpper(name) {
		if (r >= 'A' && r <= 'Z') || (r >= '0' && r <= '9') {
			b.WriteRune(r)
		} else {
			b.WriteRune('_')
		}
	}
	if v, ok := os.LookupEnv(b.String()); ok {
		return v, nil
	}
	if v, ok := os.LookupEnv("YUOS_DEV_SECRET"); ok {
		return v, nil
	}
	if os.Getenv("YUOS_DEV_MODE") == "1" {
		return "", nil
	}
	out, err := yuCallJSON("yu credential load", []string{"credential", "load", name, "--namespace", namespace}, nil)
	if err != nil {
		return "", err
	}
	if m, ok := out.Data.(map[string]any); ok {
		if val, ok := m["value"].(string); ok {
			return val, nil
		}
	}
	return "", fmt.Errorf("credential %s/%s has no value", namespace, name)
}

// PermissionScope specifies the dynamic scope for CheckPermission or RequirePermission.
type PermissionScope struct {
	Path    string
	Host    string
	Name    string
	SelfRef string
	Wild    bool
}

// CheckPermission checks whether the current session may exercise verb on scope.
func CheckPermission(verb string, scope PermissionScope) (map[string]any, error) {
	verb = strings.TrimSpace(verb)
	if verb == "" {
		return nil, errors.New("checkPermission: verb must be non-empty")
	}
	var flags [][]string
	if scope.Path != "" {
		flags = append(flags, []string{"--path", scope.Path})
	}
	if scope.Host != "" {
		flags = append(flags, []string{"--host", scope.Host})
	}
	if scope.Name != "" {
		flags = append(flags, []string{"--name", scope.Name})
	}
	if scope.SelfRef != "" {
		flags = append(flags, []string{"--self", scope.SelfRef})
	}
	if scope.Wild {
		flags = append(flags, []string{"--wild"})
	}
	if len(flags) > 1 {
		return nil, errors.New("checkPermission accepts at most one of Path / Host / Name / SelfRef / Wild")
	}
	if os.Getenv("YUOS_DEV_MODE") == "1" {
		return map[string]any{"decision": "allow", "verb": verb, "granted": true}, nil
	}
	argv := []string{"__policy", "check", verb}
	if len(flags) == 1 {
		argv = append(argv, flags[0]...)
	}
	out, err := yuCallJSON("yu __policy check", argv, nil)
	if err != nil {
		return nil, err
	}
	if m, ok := out.Data.(map[string]any); ok {
		if _, has := m["decision"].(string); has {
			return m, nil
		}
	}
	return nil, errors.New("policy check response omitted `decision`")
}

// RequirePermission requires that the current session may exercise verb on scope.
func RequirePermission(verb string, scope PermissionScope) error {
	decision, err := CheckPermission(verb, scope)
	if err != nil {
		return err
	}
	if decision["decision"] != "allow" {
		if summary, ok := decision["summary"].(string); ok && summary != "" {
			return errors.New(summary)
		}
		return fmt.Errorf("permission denied: %s", verb)
	}
	return nil
}

// LaunchApp launches a desktop application by freedesktop AppID (`desktop.launch`).
func LaunchApp(appID, uri string) error {
	appID = strings.TrimSpace(appID)
	if appID == "" {
		return errors.New("launchApp: appID must be non-empty")
	}
	argv := []string{"__desktop", "launch", "--app-id", appID}
	if strings.TrimSpace(uri) != "" {
		argv = append(argv, "--uri", strings.TrimSpace(uri))
	}
	if os.Getenv("YUOS_SESSION") == "" {
		return nil
	}
	_, err := yuCallJSON("yu __desktop launch", argv, nil)
	return err
}

// OpenURL opens an http(s) or mailto URL in the desktop browser (`desktop.launch`).
func OpenURL(url string) error {
	u := strings.TrimSpace(url)
	if !strings.HasPrefix(u, "http://") && !strings.HasPrefix(u, "https://") && !strings.HasPrefix(u, "mailto:") {
		return errors.New("openURL: url must start with http://, https://, or mailto:")
	}
	return LaunchApp("com.yuos.Browser", u)
}

// Log records a structured App log entry (`data.log.write`) to DataDir()/logs/app.jsonl and stderr.
func Log(level, message string) error {
	if strings.TrimSpace(message) == "" {
		return errors.New("log: message must be non-empty")
	}
	switch level {
	case "debug", "info", "warn", "error":
	default:
		return errors.New("log: level must be debug, info, warn, or error")
	}
	if os.Getenv("YUOS_SESSION") != "" {
		if err := RequirePermission("data.log.write", PermissionScope{Wild: true}); err != nil {
			return err
		}
	}
	dir, err := DataDir()
	if err != nil {
		return err
	}
	logDir := filepath.Join(dir, "logs")
	if err := os.MkdirAll(logDir, 0o755); err != nil {
		return err
	}
	appID := os.Getenv("YUOS_APP_ID")
	if appID == "" {
		appID = "dev"
	}
	entry, err := json.Marshal(map[string]any{
		"timestamp": time.Now().UTC().Format("2006-01-02T15:04:05Z"),
		"app":       appID,
		"level":     level,
		"message":   message,
	})
	if err != nil {
		return err
	}
	f, err := os.OpenFile(filepath.Join(logDir, "app.jsonl"), os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0o600)
	if err != nil {
		return err
	}
	defer f.Close()
	_, _ = f.Write(append(entry, '\n'))
	fmt.Fprintf(os.Stderr, "[%s] %s\n", strings.ToUpper(level), message)
	return nil
}

// HandleDesktop runs fn to open the App's desktop window.
func HandleDesktop(fn DesktopFunc) {
	if fn == nil {
		panic("yuossdk: HandleDesktop needs a function")
	}
	if desktopFunc != nil {
		panic("yuossdk: the desktop window already has a function")
	}
	desktopFunc = fn
}

func defaultUnavailable(surface, commandName string) map[string]any {
	msg := "This feature is only available in the desktop app."
	if surface == "desktop" {
		msg = "This feature is only available on the command line."
	}
	out := map[string]any{
		"available": false,
		"surface":   surface,
		"message":   msg,
	}
	if commandName != "" {
		out["command"] = commandName
	}
	return out
}

func runDesktop() error {
	if desktopFunc != nil {
		return desktopFunc()
	}
	if len(commandFuncs) > 0 {
		return json.NewEncoder(os.Stdout).Encode(defaultUnavailable("desktop", ""))
	}
	return errors.New("the App has no desktop function")
}

// RunApp calls the function Yu OS started this process for and prints a
// command's result as JSON.
func RunApp() error {
	var args []string
	hasEnvArgs := false
	if raw := os.Getenv("YUOS_ARGS_JSON"); raw != "" {
		if err := json.Unmarshal([]byte(raw), &args); err != nil {
			return fmt.Errorf("YUOS_ARGS_JSON must be an array of strings: %w", err)
		}
		hasEnvArgs = true
	}
	if os.Getenv("YUOS_APP_GUI") == "1" {
		return runDesktop()
	}
	name := os.Getenv("YUOS_COMMAND")
	if name == "" {
		cliArgs := os.Args[1:]
		if len(cliArgs) > 0 && cliArgs[0] == "--gui" {
			return runDesktop()
		}
		if len(cliArgs) > 0 {
			name = cliArgs[0]
			if !hasEnvArgs {
				args = cliArgs[1:]
			}
		}
	}
	fn := commandFuncs[name]
	if fn == nil {
		if desktopFunc != nil {
			return json.NewEncoder(os.Stdout).Encode(defaultUnavailable("cli", name))
		}
		return fmt.Errorf("the App has no function for command %q", name)
	}
	result, err := fn(args)
	if err != nil || result == nil {
		return err
	}
	return json.NewEncoder(os.Stdout).Encode(result)
}
