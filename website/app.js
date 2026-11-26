// The hero is a small working Ultron: a flip-clock island, a prompt bar on
// Ctrl+Space, and the Hands cursor ring that clicks things on the desk.
(() => {
  const $ = (id) => document.getElementById(id);
  const desk = $('desk'), island = $('island'), flip = $('flip'), say = $('say');
  const form = $('prompt'), ask = $('ask'), ring = $('ring'), dark = $('dark'), vol = $('vol');
  const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const wait = (ms) => new Promise((r) => setTimeout(r, reduced ? 0 : ms));

  // ── flip clock ──
  let shown = '';
  let timerEnd = 0;
  function render(text) {
    if (text === shown) return;
    const prev = shown;
    shown = text;
    flip.innerHTML = '';
    [...text].forEach((ch, i) => {
      const el = document.createElement('span');
      if (ch === ':') { el.className = 'colon'; el.textContent = ':'; }
      else {
        el.className = 'd' + (prev && prev[i] !== ch ? ' turn' : '');
        el.textContent = ch;
      }
      flip.appendChild(el);
    });
  }
  const pad = (n) => String(n).padStart(2, '0');
  function tick() {
    if (timerEnd) {
      const left = Math.max(0, Math.round((timerEnd - Date.now()) / 1000));
      render(`${pad(Math.floor(left / 60))}:${pad(left % 60)}`);
      if (left === 0) { timerEnd = 0; speak("Time's up.", 4000); }
    } else {
      const d = new Date();
      render(`${pad(d.getHours())}:${pad(d.getMinutes())}`);
    }
  }
  tick();
  setInterval(tick, 1000);

  // ── the island talks ──
  let hush;
  function speak(text, ms = 3200) {
    say.textContent = text;
    island.classList.add('talking');
    clearTimeout(hush);
    hush = setTimeout(() => island.classList.remove('talking'), ms);
  }

  // ── Hands cursor ring ──
  async function point(el) {
    const d = desk.getBoundingClientRect(), r = el.getBoundingClientRect();
    ring.style.left = `${r.left - d.left + r.width / 2}px`;
    ring.style.top = `${r.top - d.top + r.height / 2}px`;
    await wait(750);
  }
  async function press() {
    ring.classList.remove('click');
    void ring.offsetWidth;
    ring.classList.add('click');
    await wait(300);
  }

  // ── prompt bar ──
  function open() {
    desk.classList.add('open');
    ask.value = '';
    ask.focus({ preventScroll: true });
  }
  function close() { desk.classList.remove('open'); ask.blur(); }

  async function typeOut(text) {
    desk.classList.add('open');
    ask.value = '';
    for (const ch of text) { ask.value += ch; await wait(45); }
    await wait(350);
  }

  // ── what this demo understands ──
  const ZONES = {
    tokyo: 'Asia/Tokyo', london: 'Europe/London', paris: 'Europe/Paris', 'new york': 'America/New_York',
    dubai: 'Asia/Dubai', sydney: 'Australia/Sydney', casablanca: 'Africa/Casablanca',
    'san francisco': 'America/Los_Angeles', 'los angeles': 'America/Los_Angeles', berlin: 'Europe/Berlin',
  };
  let busy = false;

  async function run(raw) {
    const q = raw.toLowerCase().trim();
    if (!q || busy) return;
    busy = true;
    close();
    try {
      let m;
      if (/dark|light mode|appearance/.test(q)) {
        const on = !/off|light mode|disable/.test(q);
        speak(on ? 'Turning on dark appearance' : 'Turning off dark appearance');
        await point(dark); await press();
        dark.setAttribute('aria-checked', String(on));
        desk.classList.toggle('dark', on);
        await wait(400);
        speak(on ? 'Done. Dark appearance is on.' : 'Done. Dark appearance is off.');
      } else if ((m = q.match(/(\d+)\s*(min|minute|m\b|sec|second|s\b|hour|h\b)/))) {
        const n = Math.min(parseInt(m[1], 10), 600);
        const unit = m[2][0];
        const secs = unit === 's' ? n : unit === 'h' ? n * 3600 : n * 60;
        timerEnd = Date.now() + Math.min(secs, 99 * 60 + 59) * 1000;
        tick();
        speak(`Timer started: ${n} ${unit === 's' ? 'second' : unit === 'h' ? 'hour' : 'minute'}${n === 1 ? '' : 's'}`);
      } else if (/stop|cancel/.test(q) && /timer/.test(q)) {
        timerEnd = 0; tick(); speak('Timer stopped.');
      } else if (/sort|tidy|organi[sz]e|clean/.test(q) && /download|file/.test(q)) {
        speak('Sorting your downloads');
        const list = $('filelist');
        await point(list); await press();
        list.innerHTML = '<li class="folder">Photos <small>(2)</small></li><li class="folder">Documents <small>(2)</small></li>';
        await wait(300);
        speak('Done. 4 files sorted into 2 folders.');
      } else if (/time/.test(q) && (m = Object.keys(ZONES).find((z) => q.includes(z)))) {
        const t = new Intl.DateTimeFormat('en-GB', { timeZone: ZONES[m], hour: '2-digit', minute: '2-digit' }).format(new Date());
        speak(`It's ${t} in ${m.replace(/\b\w/g, (c) => c.toUpperCase())}.`, 4000);
      } else if ((m = q.match(/volume\s*(?:to\s*)?(\d+)/)) || /volume (up|down)|louder|quieter|mute/.test(q)) {
        let v = m ? Math.min(100, parseInt(m[1], 10)) : /down|quieter/.test(q) ? Number(vol.value) - 20 : /mute/.test(q) ? 0 : Number(vol.value) + 20;
        v = Math.max(0, Math.min(100, v));
        await point(vol); await press();
        vol.value = v;
        speak(`Volume ${v}%`);
      } else {
        speak('This demo only knows a few things. Try one of the suggestions.', 4200);
        await wait(600);
        open();
      }
    } finally {
      busy = false;
    }
  }

  form.addEventListener('submit', (e) => { e.preventDefault(); run(ask.value); });
  $('chips').addEventListener('click', (e) => {
    if (e.target.tagName === 'BUTTON') run(e.target.textContent);
  });
  dark.addEventListener('click', () => {
    const on = dark.getAttribute('aria-checked') !== 'true';
    dark.setAttribute('aria-checked', String(on));
    desk.classList.toggle('dark', on);
  });
  $('try').addEventListener('click', () => { desk.scrollIntoView({ behavior: reduced ? 'auto' : 'smooth', block: 'center' }); open(); });
  document.addEventListener('keydown', (e) => {
    if (e.ctrlKey && e.code === 'Space') { e.preventDefault(); desk.classList.contains('open') ? close() : open(); }
    else if (e.key === 'Escape' && desk.classList.contains('open')) close();
  });
  desk.addEventListener('click', (e) => {
    if (desk.classList.contains('open') && !e.target.closest('.prompt, .chips')) close();
  });

  // One demo when the desk first comes into view: Ultron does a task while you watch.
  let played = false;
  new IntersectionObserver(async ([entry], obs) => {
    if (!entry.isIntersecting || played) return;
    played = true;
    obs.disconnect();
    await wait(900);
    if (busy || desk.classList.contains('open')) return;
    await typeOut('turn on dark mode');
    await run('turn on dark mode');
  }, { threshold: 0.6 }).observe(desk);
})();
