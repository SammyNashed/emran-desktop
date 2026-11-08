(async () => {
  const { invoke } = window.__TAURI__.core;
  const card = await invoke('get_card');
  document.documentElement.lang = card.arabic ? 'ar' : 'en';
  document.documentElement.dir = card.arabic ? 'rtl' : 'ltr';
  document.getElementById('title').textContent = card.title;
  const stop = document.getElementById('stop');
  stop.textContent = card.stop;
  stop.addEventListener('click', () => invoke('stop_adhan'));
})();
