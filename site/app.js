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
  // Set window.CLASSIC_GITHUB_URL before this script to point every source link at the public repository.
  const source = window.CLASSIC_GITHUB_URL;
  if (source) document.querySelectorAll('[data-github-link]').forEach((link) => { link.href = source; });
})();
