(() => {
  const toggle = document.querySelector('.nav-toggle');
  const nav = document.querySelector('#site-nav');
  if (toggle && nav) {
    toggle.addEventListener('click', () => {
      const open = nav.classList.toggle('open');
      toggle.setAttribute('aria-expanded', String(open));
      toggle.textContent = open ? 'Close' : 'Menu';
    });
  }
  document.querySelectorAll('[data-year]').forEach((item) => { item.textContent = new Date().getFullYear(); });
  // A custom deployment may override this before the script runs.
  const source = window.CLASSIC_GITHUB_URL || 'https://github.com/Razin-developer/classic-studio';
  if (source) document.querySelectorAll('[data-github-link]').forEach((link) => { link.href = source; });
})();
