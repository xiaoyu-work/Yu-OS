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
)

// CommandFunc receives a command's validated arguments and returns a JSON value.
type CommandFunc func(args []string) (any, error)

// DesktopFunc opens the App's desktop window.
type DesktopFunc func() error

var (
	commandFuncs = map[string]CommandFunc{}
	desktopFunc  DesktopFunc
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
