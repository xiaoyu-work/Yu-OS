async function checkPackageSource() {
  const notice = document.querySelector("[data-source-status]");
  try {
    const response = await fetch("../apt/dists/trixie/InRelease", {
      method: "HEAD",
      cache: "no-store",
      signal: AbortSignal.timeout(6000),
    });
    if (response.ok) return;
    notice.textContent = "The signed Yu OS package source is currently unavailable. Try the Ubuntu installation again later.";
  } catch {
    notice.textContent = "Could not check the Yu OS package source. Stop if the package key or index cannot be verified.";
  }
  notice.hidden = false;
}

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

void checkPackageSource();
