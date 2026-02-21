(() => {
  const navs = Array.from(document.querySelectorAll('[data-nav-scope]'));
  if (!navs.length) {
    return;
  }

  // Use stable nav item keys; labels can change without breaking selectors.
  const navData = navs.map((nav) => {
    const items = Array.from(nav.querySelectorAll('[data-nav-item]'));
    return { nav, items, scope: nav.dataset.navScope };
  });

  // Toggles and menus are matched via shared data-nav-* keys.
  const getToggle = (item) => item.querySelector('[data-nav-toggle]');
  const getMenu = (item) => item.querySelector('[data-nav-menu]');
  const isOpen = (item) => item.dataset.open === 'true';
  const openItem = (item) => {
    const menu = getMenu(item);
    const toggle = getToggle(item);
    if (!menu || !toggle) {
      return;
    }
    menu.classList.remove('hidden');
    toggle.setAttribute('aria-expanded', 'true');
    item.dataset.open = 'true';
  };
  const closeItem = (item) => {
    const menu = getMenu(item);
    const toggle = getToggle(item);
    if (!menu || !toggle) {
      return;
    }
    menu.classList.add('hidden');
    toggle.setAttribute('aria-expanded', 'false');
    item.dataset.open = 'false';
  };

  navData.forEach(({ nav, items, scope }) => {
    const closeAll = () => items.forEach(closeItem);

    items.forEach((item) => {
      const toggle = getToggle(item);
      const menu = getMenu(item);
      if (!toggle || !menu) {
        return;
      }

      closeItem(item);

      toggle.addEventListener('click', (event) => {
        event.preventDefault();
        event.stopPropagation();

        items.forEach((other) => {
          if (other !== item) {
            closeItem(other);
          }
        });

        if (isOpen(item)) {
          closeItem(item);
        } else {
          openItem(item);
        }
      });

      if (scope === 'desktop') {
        item.addEventListener('mouseenter', () => {
          openItem(item);
        });
        item.addEventListener('mouseleave', () => {
          closeItem(item);
        });
        item.addEventListener('focusin', () => {
          openItem(item);
        });
        item.addEventListener('focusout', (event) => {
          if (!item.contains(event.relatedTarget)) {
            closeItem(item);
          }
        });
      }
    });

    nav.addEventListener('click', (event) => {
      event.stopPropagation();
    });

    document.addEventListener('click', (event) => {
      if (!nav.contains(event.target)) {
        closeAll();
      }
    });
  });

  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') {
      navData.forEach(({ items }) => items.forEach(closeItem));
    }
  });
})();
