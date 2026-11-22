(() => {
  const root = document.querySelector('.agent');
  const typed = root.querySelector('.typed');
  const picks = [...root.querySelectorAll('.pick')];
  const toggle = root.querySelector('.toggle');
  const sentences = [...root.querySelectorAll('.sr li')].map((li) => li.textContent);
  let index = 0;
  let shown = 0;
  let timer = 0;
  let paused = false;
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
  const select = (next) => {
    clearTimeout(timer);
    root.classList.remove('speaking');
    index = next;
    picks.forEach((pick, i) => pick.setAttribute('aria-pressed', String(i === next)));
    if (paused) {
      typed.textContent = sentences[index];
      return;
    }
    shown = 0;
    type();
  };
  picks.forEach((pick, i) => pick.addEventListener('click', () => select(i)));
  toggle.addEventListener('click', () => {
    paused = !paused;
    root.classList.toggle('paused', paused);
    toggle.setAttribute('aria-label', paused ? 'Play' : 'Pause');
    select(index);
  });
  new IntersectionObserver((entries, observer) => {
    if (!entries.some((entry) => entry.intersectionRatio >= 0.5)) return;
    observer.disconnect();
    select(0);
  }, { threshold: 0.5 }).observe(root.querySelector('.caption'));
})();
