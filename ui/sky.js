// The sky follows the real prayer times. Port of the phone's ui/design/Sky.kt, using the same palette,
// the same key moments and the same readability rules (tame and lift).
(() => {
  const hex = (h) => [(h >> 16 & 255) / 255, (h >> 8 & 255) / 255, (h & 255) / 255];
  const palette = (top, mid, low, glow, stars) => ({ top: hex(top), mid: hex(mid), low: hex(low), glow: hex(glow), stars });
  const NIGHT = palette(0x040A18, 0x071626, 0x123F55, 0x1F6B80, 1);
  const FAJR = palette(0x0C183A, 0x6A5A8E, 0xF2856A, 0xFFB38A, 0.5);
  const SUNRISE = palette(0x35588F, 0xD9A7A0, 0xFFC890, 0xFFD9A0, 0);
  const DAY = palette(0x7FB8EE, 0xAED9FB, 0xEAF6FF, 0xFFFFFF, 0);
  const ASR = palette(0x86B1D6, 0xE4CD9F, 0xE69F3F, 0xFFD27A, 0);
  const MAGHRIB = palette(0x0D1C3F, 0x5D3B59, 0xD84A2F, 0xFF8A4C, 0.35);
  const WHITE = [1, 1, 1];
  const BLACK = [0, 0, 0];
  const INK_DARK = hex(0x0B1B33);

  const toLinear = (c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
  const toGamma = (c) => (c <= 0.0031308 ? 12.92 * c : 1.055 * Math.max(c, 0) ** (1 / 2.4) - 0.055);
  const luminance = ([r, g, b]) => 0.2126 * toLinear(r) + 0.7152 * toLinear(g) + 0.0722 * toLinear(b);

  function toOklab([sr, sg, sb]) {
    const r = toLinear(sr), g = toLinear(sg), b = toLinear(sb);
    const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
    const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
    const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
    return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s, 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s];
  }

  function fromOklab([L, a, b]) {
    const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
    const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
    const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
    const clamp = (v) => Math.min(1, Math.max(0, toGamma(v)));
    return [clamp(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s), clamp(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s), clamp(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s)];
  }

  // Compose interpolates colours in Oklab, so this does too.
  function mixColor(a, b, t) {
    const A = toOklab(a), B = toOklab(b);
    return fromOklab(A.map((v, i) => v + (B[i] - v) * t));
  }

  const smooth = (x) => { const t = Math.min(1, Math.max(0, x)); return t * t * (3 - 2 * t); };
  const between = (x, a, b) => (b <= a ? (x >= b ? 1 : 0) : smooth((x - a) / (b - a)));
  const mixPalette = (a, b, t) => ({
    top: mixColor(a.top, b.top, t), mid: mixColor(a.mid, b.mid, t), low: mixColor(a.low, b.low, t),
    glow: mixColor(a.glow, b.glow, t), stars: a.stars + (b.stars - a.stars) * t,
  });

  function compute(nowMs, t) {
    const min = (ms) => ms / 60000;
    const n = min(nowMs);
    const keys = [
      [min(t.fajr) - 50, NIGHT], [min(t.fajr), FAJR], [min(t.sunrise), SUNRISE], [min(t.sunrise) + 80, DAY],
      [min(t.dhuhr), DAY], [min(t.asr), ASR], [min(t.maghrib), MAGHRIB], [min(t.isha), NIGHT], [min(t.tomorrowFajr) - 50, NIGHT],
    ].sort((a, b) => a[0] - b[0]);
    let p;
    if (n <= keys[0][0]) p = keys[0][1];
    else if (n >= keys[keys.length - 1][0]) p = keys[keys.length - 1][1];
    else {
      let i = 0;
      keys.forEach((k, index) => { if (k[0] <= n) i = index; });
      p = mixPalette(keys[i][1], keys[i + 1][1], between(n, keys[i][0], keys[i + 1][0]));
    }
    const darkInk = between(luminance(p.mid), 0.16, 0.32);
    const whiteInk = 1 - darkInk;
    const tame = (c, maxLum) => {
      const l = luminance(c);
      if (l <= maxLum) return c;
      return mixColor(c, BLACK, (1 - (maxLum / l) ** (1 / 2.2)) * whiteInk);
    };
    const lift = (c, minLum, toward) => {
      if (luminance(c) >= minLum || darkInk <= 0) return c;
      let lo = 0, hi = 1;
      for (let i = 0; i < 12; i++) {
        const m = (lo + hi) / 2;
        if (luminance(mixColor(c, toward, m)) >= minLum) hi = m; else lo = m;
      }
      return mixColor(c, toward, hi * darkInk);
    };
    return {
      top: lift(p.top, 0.3, mixColor(p.mid, WHITE, 0.35)), mid: p.mid, low: tame(p.low, 0.12), glow: tame(p.glow, 0.2),
      stars: p.stars, darkInk, ink: mixColor(WHITE, INK_DARK, darkInk), glass: 0.12 + 0.3 * darkInk,
    };
  }

  const css = (c, alpha = 1) => `rgb(${c.map((v) => Math.round(v * 255)).join(' ')} / ${alpha})`;

  function apply(sky) {
    const s = document.documentElement.style;
    s.setProperty('--top', css(sky.top));
    s.setProperty('--mid', css(sky.mid));
    s.setProperty('--low', css(sky.low));
    s.setProperty('--glow', css(sky.glow, 0.3));
    s.setProperty('--ink', css(sky.ink));
    s.setProperty('--ink-soft', css(sky.ink, 0.72));
    s.setProperty('--glass-tint', `rgb(255 255 255 / ${sky.glass})`);
    s.setProperty('--frost', sky.darkInk > 0.5 ? 'rgb(255 255 255 / .28)' : 'rgb(8 16 40 / .34)');
    s.setProperty('--stroke', css(sky.ink, 0.16));
    s.setProperty('--selected', css(sky.ink, 0.16));
    s.setProperty('--stars', String(sky.stars));
  }

  function drawStars() {
    const canvas = document.getElementById('stars');
    const ratio = window.devicePixelRatio || 1;
    canvas.width = innerWidth * ratio;
    canvas.height = innerHeight * ratio;
    const g = canvas.getContext('2d');
    let seed = 7;
    const random = () => { seed = (seed + 0x6D2B79F5) | 0; let t = Math.imul(seed ^ (seed >>> 15), 1 | seed); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; };
    for (let i = 0; i < 150; i++) {
      const x = random() * canvas.width, y = random() * 0.66 * canvas.height, r = (0.5 + random() * 1.1) * ratio, a = 0.35 + random() * 0.65;
      g.fillStyle = `rgb(255 255 255 / ${a})`;
      g.beginPath(); g.arc(x, y, r, 0, Math.PI * 2); g.fill();
    }
  }

  window.EmranSky = { compute, apply, drawStars };
})();
