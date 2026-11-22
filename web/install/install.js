if (navigator.clipboard) {
  const status = document.querySelector("[data-copy-status]");
  for (const figure of document.querySelectorAll(".command")) {
    const code = figure.querySelector("code");
    const button = document.createElement("button");
    button.type = "button";
    button.className = "copy-command";
    button.textContent = "Copy";
    let timer;
    button.addEventListener("click", async () => {
      clearTimeout(timer);
      try {
        await navigator.clipboard.writeText(code.textContent);
        button.textContent = "Copied";
        status.textContent = "Commands copied.";
      } catch {
        button.textContent = "Copy failed";
        status.textContent = "Could not copy the commands. Select the text to copy it manually.";
      }
      timer = setTimeout(() => { button.textContent = "Copy"; }, 2000);
    });
    figure.append(button);
  }
}
