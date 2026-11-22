(() => {
  const root = document.querySelector('.agent');
  const typed = root.querySelector('.typed');
  const picks = [...root.querySelectorAll('.pick')];
  const sentences = [...root.querySelectorAll('.sr li')].map((li) => li.textContent);
  let index = 0;
  let shown = 0;
  let timer = 0;
  const type = () => {
    const sentence = sentences[index];
    if (shown <= sentence.length) {
      typed.textContent = sentence.slice(0, shown);
      shown += 1;
      root.classList.add('speaking');
      timer = setTimeout(type, 32);
      return;
    }
    root.classList.remove('speaking');
    timer = setTimeout(() => select((index + 1) % sentences.length), 2800);
  };
  const select = (next, animate = true) => {
    clearTimeout(timer);
    root.classList.remove('speaking');
    index = next;
    picks.forEach((pick, i) => pick.setAttribute('aria-pressed', String(i === next)));
    if (!animate) {
      typed.textContent = sentences[index];
      return;
    }
    shown = 0;
    type();
  };
  const observer = new IntersectionObserver((entries) => {
    if (!entries.some((entry) => entry.intersectionRatio >= 0.5)) return;
    observer.disconnect();
    select(0);
  }, { threshold: 0.5 });
  picks.forEach((pick, i) => pick.addEventListener('click', () => {
    observer.disconnect();
    select(i, false);
  }));
  observer.observe(root.querySelector('.caption'));
})();
