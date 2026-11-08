const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = (id) => document.getElementById(id);

let snap = null;
let offset = 0;
let cities = null;
let dialogUpdate = null;

const ar = () => snap.settings.lang !== 'en';
const tr = (arabic, english) => (ar() ? arabic : english);

const PRAYERS = { Fajr: ['الفجر', 'Fajr'], Sunrise: ['الشروق', 'Sunrise'], Dhuhr: ['الظهر', 'Dhuhr'], Asr: ['العصر', 'Asr'], Maghrib: ['المغرب', 'Maghrib'], Isha: ['العشاء', 'Isha'] };
const FIVE = ['Fajr', 'Dhuhr', 'Asr', 'Maghrib', 'Isha'];
const OFFSET_KEYS = ['Fajr', 'Sunrise', 'Dhuhr', 'Asr', 'Maghrib', 'Isha'];
const WEEKDAYS = { ar: ['الإثنين', 'الثلاثاء', 'الأربعاء', 'الخميس', 'الجمعة', 'السبت', 'الأحد'], en: ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'] };
const MONTHS = {
  ar: ['يناير', 'فبراير', 'مارس', 'أبريل', 'مايو', 'يونيو', 'يوليو', 'أغسطس', 'سبتمبر', 'أكتوبر', 'نوفمبر', 'ديسمبر'],
  en: ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'],
};
const HIJRI = {
  ar: ['محرم', 'صفر', 'ربيع الأول', 'ربيع الآخر', 'جمادى الأولى', 'جمادى الآخرة', 'رجب', 'شعبان', 'رمضان', 'شوال', 'ذو القعدة', 'ذو الحجة'],
  en: ['Muharram', 'Safar', 'Rabi al-Awwal', 'Rabi al-Thani', 'Jumada al-Ula', 'Jumada al-Akhirah', 'Rajab', 'Shaban', 'Ramadan', 'Shawwal', 'Dhu al-Qidah', 'Dhu al-Hijjah'],
};
const METHODS = [
  ['EGYPTIAN', 'الهيئة المصرية العامة للمساحة', 'Egyptian General Authority'], ['UMM_AL_QURA', 'جامعة أم القرى (مكة المكرمة)', 'Umm Al-Qura (Mecca)'],
  ['MWL', 'رابطة العالم الإسلامي', 'Muslim World League'], ['ISNA', 'الجمعية الإسلامية لأمريكا الشمالية', 'Islamic Society of North America'],
  ['KARACHI', 'جامعة العلوم الإسلامية بكراتشي', 'University of Islamic Sciences, Karachi'], ['DUBAI', 'دبي (الإمارات)', 'Dubai (UAE)'],
  ['JORDAN', 'وزارة الأوقاف الأردنية', 'Ministry of Awqaf, Jordan'], ['MOROCCO', 'وزارة الأوقاف المغربية', 'Ministry of Habous, Morocco'], ['TUNISIA', 'تونس', 'Tunisia'],
  ['JAKIM', 'ماليزيا (جاكيم)', 'JAKIM (Malaysia)'], ['KEMENAG', 'إندونيسيا (وزارة الشؤون الدينية)', 'Kemenag (Indonesia)'], ['QATAR', 'قطر', 'Qatar'], ['KUWAIT', 'الكويت', 'Kuwait'],
  ['SINGAPORE', 'سنغافورة (المجلس الإسلامي)', 'Singapore (MUIS)'], ['TURKEY', 'الديانة التركية', 'Diyanet (Turkey)'], ['TEHRAN', 'معهد الجيوفيزياء، طهران', 'Institute of Geophysics, Tehran'],
  ['JAFARI', 'الجعفري (الإثنا عشري)', "Shia Ithna Ashari (Ja'fari)"],
];
const SCHOOLS = [['SHAFII', 'الجمهور (شافعي، مالكي، حنبلي)', "Standard (Shafi'i, Maliki, Hanbali)"], ['HANAFI', 'المذهب الحنفي', 'Hanafi School']];
const HIGH_LAT = [['AUTOMATIC', 'تلقائي (موصى به)', 'Automatic (recommended)'], ['MIDDLE_OF_NIGHT', 'منتصف الليل', 'Middle of the night'], ['ONE_SEVENTH_OF_NIGHT', 'سُبع الليل', 'One-seventh of the night'], ['ANGLE_BASED', 'حسب الزاوية (تناسبي)', 'Angle-based (proportional)'], ['NONE', 'بدون تعديل (فلكي بحت)', 'None (astronomical only)']];
const VOICES = [['ahmad_alnafees', 'أحمد النفيس', 'Ahmad Alnafys'], ['mishary_alafasy', 'مشاري العفاسي', 'Mishary Alafasy'], ['saad_alghamdi', 'سعد الغامدي', 'Saad Alghamdy']];

const lang = () => (ar() ? 'ar' : 'en');
const label = (table, key) => { const row = table.find((r) => r[0] === key); return row ? (ar() ? row[1] : row[2]) : key; };
const options = (table) => table.map((r) => [r[0], ar() ? r[1] : r[2]]);
function prayerName(key, weekday) {
  if (key === 'Dhuhr' && weekday === 5) return tr('الجمعة', "Jumu'ah");
  return PRAYERS[key][ar() ? 0 : 1];
}
const two = (n) => String(n).padStart(2, '0');

function cityLabel() {
  const s = snap.settings;
  if (s.city === 'custom') return tr('موقع مخصص', 'Custom location');
  const city = cities && cities.find((c) => c.name === s.city);
  const full = city ? (ar() ? city.name_ar : city.name) : s.city;
  return full.split(/[،,]/)[0].trim();
}

function hijriLine(day) {
  if (day.hijri_year == null) return WEEKDAYS[lang()][day.weekday - 1];
  const month = HIJRI[lang()][day.hijri_month - 1];
  return ar() ? `${WEEKDAYS.ar[day.weekday - 1]} ${day.hijri_day} ${month} ${day.hijri_year}` : `${WEEKDAYS.en[day.weekday - 1]}, ${day.hijri_day} ${month} ${day.hijri_year}`;
}
const gregorianLine = (day) => `${day.day} ${MONTHS[lang()][day.month - 1]} ${day.year}`;

function dayTitle(day) {
  if (day.offset === 0) return tr('اليوم', 'Today');
  if (day.offset === 1) return tr('غداً', 'Tomorrow');
  if (day.offset === -1) return tr('أمس', 'Yesterday');
  return `${WEEKDAYS[lang()][day.weekday - 1]} ${day.day} ${MONTHS[lang()][day.month - 1]}`;
}

function setClock(node, text) {
  const [time, suffix] = text.split(' ');
  node.textContent = time;
  if (suffix) node.append(el('small', '', suffix));
}

function renderHome() {
  const { settings: s, next, today, shown, playing } = snap;
  $('place-name').textContent = cityLabel();
  $('hijri').textContent = hijriLine(today);
  $('gregorian').textContent = gregorianLine(today);
  $('language').textContent = ar() ? 'English' : 'العربية';
  $('open-settings').setAttribute('aria-label', tr('الإعدادات', 'Settings'));
  const bell = $('bell');
  bell.setAttribute('aria-checked', String(!s.muted));
  bell.setAttribute('aria-label', tr('صوت الأذان', 'Adhan sound'));
  $('bell-glyph').setAttribute('href', s.muted ? '#i-bell-off' : '#i-bell');
  $('count-label').textContent = next.key === 'Sunrise' ? tr('باقي للشروق', 'Time to sunrise') : tr('باقي للأذان', 'Time to adhan');
  $('next-name').textContent = prayerName(next.key, next.weekday);
  $('next-time').textContent = next.clock;
  const stop = $('stop-home');
  stop.hidden = !playing;
  stop.textContent = tr('إيقاف الأذان', 'Stop Adhan');
  $('prev-day').setAttribute('aria-label', tr('اليوم السابق', 'Previous day'));
  $('next-day').setAttribute('aria-label', tr('اليوم التالي', 'Next day'));
  $('day-title').innerHTML = '';
  const title = document.createElement('b');
  title.textContent = shown.estimated ? `${dayTitle(shown)} · ${tr('تقديري', 'Estimated')}` : dayTitle(shown);
  $('day-title').append(title);
  const times = $('times');
  times.replaceChildren(...FIVE.map((key) => {
    const row = shown.times.find((t) => t.key === key);
    const cell = document.createElement('div');
    cell.className = `time${shown.offset === 0 && next.key === key ? ' now' : ''}`;
    const name = document.createElement('div'); name.className = 'name'; name.textContent = prayerName(key, shown.weekday);
    const clock = document.createElement('div'); clock.className = 'clock'; clock.dir = 'ltr'; setClock(clock, row.clock);
    cell.append(name, clock);
    return cell;
  }));
}

function row(parent, title, value, onClick) {
  const b = document.createElement('button');
  b.className = 'row'; b.type = 'button';
  const t = document.createElement('span'); t.className = 'title'; t.textContent = title;
  const v = document.createElement('span'); v.className = 'value'; v.textContent = value ?? '';
  b.append(t, v);
  b.insertAdjacentHTML('beforeend', '<svg class="icon"><use href="#i-chevron"/></svg>');
  b.addEventListener('click', onClick);
  parent.append(b);
}

function toggleRow(parent, title, on, onClick) {
  const b = document.createElement('button');
  b.className = 'row'; b.type = 'button'; b.setAttribute('role', 'switch'); b.setAttribute('aria-checked', String(on));
  const t = document.createElement('span'); t.className = 'title'; t.textContent = title;
  const sw = document.createElement('span'); sw.className = 'switch';
  b.append(t, sw);
  b.addEventListener('click', onClick);
  parent.append(b);
}

function renderSettings() {
  const s = snap.settings;
  $('settings-title').textContent = tr('الإعدادات', 'Settings');
  $('back').setAttribute('aria-label', tr('رجوع', 'Back'));
  $('group-times').textContent = tr('حساب المواقيت', 'Prayer calculation');
  $('group-adhan').textContent = tr('الأذان', 'Adhan');
  $('group-general').textContent = tr('عام', 'General');

  const times = $('rows-times'); times.replaceChildren();
  row(times, tr('الموقع', 'Location'), cityLabel(), openLocation);
  row(times, tr('الطريقة', 'Method'), label(METHODS, s.method), () => choice(tr('طريقة الحساب', 'Calculation method'), options(METHODS), s.method, (v) => save({ method: v })));
  row(times, tr('العصر', 'Asr'), label(SCHOOLS, s.school), () => choice(tr('وقت العصر', 'Asr time'), options(SCHOOLS), s.school, (v) => save({ school: v })));
  if (Math.abs(s.lat) > 48) row(times, tr('خطوط العرض العليا', 'High latitude'), label(HIGH_LAT, s.high_lat), () => choice(tr('خطوط العرض العليا', 'High latitude'), options(HIGH_LAT), s.high_lat, (v) => save({ high_lat: v })));
  row(times, tr('ضبط الدقائق يدوياً', 'Fine-tune minutes'), Object.values(s.offsets).some((m) => m !== 0) ? tr('مفعّل', 'On') : '', openOffsets);

  const adhan = $('rows-adhan'); adhan.replaceChildren();
  toggleRow(adhan, tr('صوت الأذان', 'Adhan sound'), !s.muted, () => invoke('toggle_mute'));
  row(adhan, tr('المؤذّن', 'Muezzin'), label(VOICES, s.voice), () => choice(tr('المؤذّن', 'Muezzin'), options(VOICES), s.voice, (v) => save({ voice: v }), 'Maghrib'));
  row(adhan, tr('طول الأذان', 'Adhan length'), s.full ? tr('الأذان كاملًا', 'The whole adhan') : tr('أوّله فقط', 'Opening only'), () => choice(tr('طول الأذان', 'Adhan length'), [['short', tr('أوّله فقط', 'Opening only')], ['full', tr('الأذان كاملًا', 'The whole adhan')]], s.full ? 'full' : 'short', (v) => save({ full: v === 'full' }), 'Maghrib'));
  const fajr = [['', tr('الأذان', 'The adhan')], ['mishary_alafasy', tr('«الصلاة خير من النوم»', '“Prayer is better than sleep”')]];
  row(adhan, tr('صوت الفجر', 'Fajr sound'), fajr.find((f) => f[0] === s.fajr_sound)[1], () => choice(tr('صوت الفجر', 'Fajr sound'), fajr, s.fajr_sound, (v) => save({ fajr_sound: v }), 'Fajr'));
  row(adhan, tr('حدّ الصوت', 'Volume limit'), `${s.loudness}%`, openLoudness);

  const general = $('rows-general'); general.replaceChildren();
  row(general, tr('صيغة الوقت', 'Time format'), s.clock24 ? tr('24 ساعة', '24-hour') : tr('12 ساعة', '12-hour'), () => choice(tr('صيغة الوقت', 'Time format'), [['24', tr('24 ساعة', '24-hour')], ['12', tr('12 ساعة', '12-hour')]], s.clock24 ? '24' : '12', (v) => save({ clock24: v === '24' })));
  toggleRow(general, tr('التشغيل مع النظام', 'Start with the system'), s.autostart, () => invoke('set_autostart', { on: !s.autostart }).then(refresh));
}

function render() {
  document.documentElement.lang = lang();
  document.documentElement.dir = ar() ? 'rtl' : 'ltr';
  renderHome();
  if (!$('settings').hidden) renderSettings();
  if (dialogUpdate) dialogUpdate();
  updateSky();
  tick();
}

function updateSky() {
  const at = (key) => snap.today.times.find((t) => t.key === key).epoch_ms;
  EmranSky.apply(EmranSky.compute(Date.now(), { fajr: at('Fajr'), sunrise: at('Sunrise'), dhuhr: at('Dhuhr'), asr: at('Asr'), maghrib: at('Maghrib'), isha: at('Isha'), tomorrowFajr: snap.tomorrow_fajr_ms }));
}

let refreshing = false;
function tick() {
  if (!snap) return;
  const left = Math.max(0, Math.floor((snap.next.epoch_ms - Date.now()) / 1000));
  $('countdown').textContent = `${two(Math.floor(left / 3600))}:${two(Math.floor(left / 60) % 60)}:${two(left % 60)}`;
  if (left === 0 && !refreshing && snap.next.epoch_ms <= Date.now()) {
    refreshing = true;
    refresh().finally(() => { setTimeout(() => { refreshing = false; }, 1000); });
  }
}

async function refresh() {
  snap = await invoke('get_snapshot', { offset });
  render();
}

async function save(patch) {
  await invoke('update_settings', { new: { ...snap.settings, ...patch } });
  await refresh();
}

function showDialog(build) {
  const dialog = $('dialog');
  dialogUpdate = () => { const list = dialog.querySelector('.list'); const top = list ? list.scrollTop : 0; dialog.replaceChildren(); build(dialog); const next = dialog.querySelector('.list'); if (next) next.scrollTop = top; };
  dialogUpdate();
  $('overlay').hidden = false;
}
function closeDialog() { $('overlay').hidden = true; dialogUpdate = null; }

function el(tag, className, text) { const e = document.createElement(tag); if (className) e.className = className; if (text != null) e.textContent = text; return e; }
function button(className, text, onClick) { const b = el('button', className, text); b.type = 'button'; b.addEventListener('click', onClick); return b; }
const CHECK = '<svg class="icon"><use href="#i-check"/></svg>';

function soundFooter(prayer) {
  const footer = el('div', 'footer');
  if (prayer) {
    footer.append(button('action primary', tr('جرّب', 'Test'), () => invoke('preview_adhan', { prayer })));
    footer.append(button('action', tr('إيقاف', 'Stop'), () => invoke('stop_adhan')));
  }
  footer.append(button('action', tr('تم', 'Done'), closeDialog));
  return footer;
}

function choice(title, items, initial, pick, testPrayer) {
  let selected = initial;
  showDialog((d) => {
    d.append(el('h3', '', title));
    const list = el('div', 'list');
    for (const [key, text] of items) {
      const b = button(`choice${key === selected ? ' selected' : ''}`, text, async () => {
        selected = key;
        await pick(key);
        if (!testPrayer) closeDialog();
      });
      if (key === selected) b.insertAdjacentHTML('beforeend', CHECK);
      list.append(b);
    }
    d.append(list, soundFooter(testPrayer));
  });
}

function openLoudness() {
  showDialog((d) => {
    d.append(el('h3', '', tr('حدّ الصوت', 'Volume limit')));
    const line = el('div', 'loud');
    const value = el('span', '', `${snap.settings.loudness}%`); value.dir = 'ltr';
    const input = document.createElement('input');
    input.type = 'range'; input.min = 20; input.max = 100; input.value = snap.settings.loudness;
    input.setAttribute('aria-label', tr('حدّ الصوت', 'Volume limit'));
    input.addEventListener('input', () => { value.textContent = `${input.value}%`; });
    input.addEventListener('change', () => save({ loudness: Number(input.value) }));
    line.append(input, value);
    d.append(line, soundFooter('Maghrib'));
  });
}

function openOffsets() {
  showDialog((d) => {
    d.append(el('h3', '', tr('ضبط الدقائق يدوياً', 'Fine-tune minutes')));
    const list = el('div', 'list');
    for (const key of OFFSET_KEYS) {
      const field = key.toLowerCase();
      const minutes = snap.settings.offsets[field];
      const change = (delta) => {
        const value = Math.min(120, Math.max(-120, snap.settings.offsets[field] + delta));
        save({ offsets: { ...snap.settings.offsets, [field]: value } });
      };
      const line = el('div', 'offset');
      const clock = el('span', 'clock', snap.today.times.find((t) => t.key === key).clock); clock.dir = 'ltr';
      const delta = el('span', 'delta', minutes === 0 ? tr('بلا تعديل', 'No change') : `⁦${minutes > 0 ? '+' : ''}${minutes}⁩ ${tr('د', 'min')}`);
      line.append(el('span', '', prayerName(key, snap.today.weekday)), clock, button('step', '−', () => change(-1)), delta, button('step', '+', () => change(1)));
      list.append(line);
    }
    d.append(list, soundFooter());
  });
}

async function openLocation() {
  let term = '';
  showDialog((d) => {
    d.append(el('h3', '', tr('الموقع', 'Location')));
    const search = document.createElement('input');
    search.type = 'text'; search.placeholder = tr('بحث', 'Search'); search.value = term;
    const list = el('div', 'list');
    const fill = () => {
      const q = search.value.trim().toLowerCase();
      const found = cities.filter((c) => !q || c.name.toLowerCase().includes(q) || c.name_ar.includes(q));
      list.replaceChildren(...found.map((c) => {
        const b = button(`choice${c.name === snap.settings.city ? ' selected' : ''}`, ar() ? c.name_ar : c.name, async () => {
          await save({ city: c.name, lat: c.lat, lon: c.lon, zone: c.zone });
          closeDialog();
        });
        if (c.name === snap.settings.city) b.insertAdjacentHTML('beforeend', CHECK);
        return b;
      }));
    };
    search.addEventListener('input', () => { term = search.value; fill(); });
    fill();

    const coords = el('div', 'coords');
    const field = (text, value) => { const l = el('label', '', text); const i = document.createElement('input'); i.type = 'text'; i.dir = 'ltr'; i.inputMode = 'decimal'; i.value = value; l.append(i); return [l, i]; };
    const [latLabel, lat] = field(tr('خط العرض', 'Latitude'), snap.settings.city === 'custom' ? snap.settings.lat : '');
    const [lonLabel, lon] = field(tr('خط الطول', 'Longitude'), snap.settings.city === 'custom' ? snap.settings.lon : '');
    coords.append(latLabel, lonLabel, button('action', tr('حفظ', 'Save'), async () => {
      const la = Number(lat.value), lo = Number(lon.value);
      if (lat.value.trim() === '' || lon.value.trim() === '' || !Number.isFinite(la) || !Number.isFinite(lo) || Math.abs(la) > 90 || Math.abs(lo) > 180) return;
      const zone = await invoke('system_zone');
      await save({ city: 'custom', lat: la, lon: lo, zone });
      closeDialog();
    }));
    d.append(search, list, coords, soundFooter());
    if (document.activeElement === document.body) search.focus({ preventScroll: true });
  });
}

function showPage(name) {
  $('home').hidden = name !== 'home';
  $('settings').hidden = name !== 'settings';
  if (name === 'settings') renderSettings();
}

function stepDay(delta) { offset += delta; refresh(); }

$('open-settings').addEventListener('click', () => showPage('settings'));
$('back').addEventListener('click', () => showPage('home'));
$('place').addEventListener('click', openLocation);
$('bell').addEventListener('click', () => invoke('toggle_mute'));
$('language').addEventListener('click', () => save({ lang: ar() ? 'en' : 'ar' }));
$('stop-home').addEventListener('click', () => invoke('stop_adhan'));
$('prev-day').addEventListener('click', () => stepDay(-1));
$('next-day').addEventListener('click', () => stepDay(1));
$('day-title').addEventListener('click', () => { offset = 0; refresh(); });
$('overlay').addEventListener('click', (e) => { if (e.target === $('overlay')) closeDialog(); });
window.addEventListener('keydown', (e) => { if (e.key === 'Escape' && !$('overlay').hidden) closeDialog(); });
window.addEventListener('resize', EmranSky.drawStars);

(async () => {
  cities = await invoke('get_cities');
  EmranSky.drawStars();
  await refresh();
  await listen('changed', refresh);
  setInterval(tick, 1000);
  setInterval(updateSky, 60000);
})();
