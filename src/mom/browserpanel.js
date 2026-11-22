// The MyOpenMath helper panel (web app): a live view of the helper's browser (src/mom/browserd.rs)
// that the teacher can watch and take over, with the course and assessment pickers and the Add
// buttons. window.dadBrowser.open({server, token, question, name}).
(function () {
  if (window.dadBrowser) return;
  var ID = 'dad_mom_helper';
  var css = '#' + ID + '{position:fixed;inset:0;z-index:40;display:flex;flex-direction:column;background:#f7f4ed;font:15px "Source Sans 3",system-ui,sans-serif;color:#241f16}'
    + '#' + ID + ' .bar{display:flex;flex-wrap:wrap;gap:10px;align-items:center;padding:10px 16px;border-bottom:1px solid #d8cfbf;background:#fffdf8}'
    + '#' + ID + ' .grow{flex:1;min-width:200px}'
    + '#' + ID + ' button{font:600 15px "Source Sans 3",system-ui,sans-serif;padding:7px 16px;border-radius:10px;cursor:pointer;border:1px solid #cbbfa8;background:#fffdf8;color:#241f16}'
    + '#' + ID + ' button.primary{background:#2d5e45;border-color:#2d5e45;color:#fff}'
    + '#' + ID + ' button:disabled{opacity:.5;cursor:default}'
    + '#' + ID + ' select{font:15px "Source Sans 3",system-ui,sans-serif;padding:6px 8px;border-radius:8px;border:1px solid #cbbfa8;background:#fff;max-width:280px}'
    + '#' + ID + ' .pill{padding:3px 12px;border-radius:999px;font-weight:700;font-size:13px}'
    + '#' + ID + ' .you{background:#fff1c2;color:#6b4e00}#' + ID + ' .helper{background:#dcefe3;color:#2d5e45}#' + ID + ' .nobody{background:#eee;color:#555}'
    + '#' + ID + ' .status{color:#5a4f3c}#' + ID + ' .bad{color:#a8322d;font-weight:600}#' + ID + ' .good{color:#2d5e45;font-weight:600}'
    + '#' + ID + ' .view{flex:1;position:relative;overflow:hidden;background:#e9e4da}'
    + '#' + ID + ' canvas{position:absolute;left:0;top:0;outline:none;background:#fff;box-shadow:0 1px 6px rgba(0,0,0,.15)}'
    + '#' + ID + ' textarea.keys{position:absolute;left:-9999px;top:0;width:1px;height:1px;opacity:0}';

  function el(tag, attrs, text) {
    var e = document.createElement(tag);
    for (var k in (attrs || {})) e.setAttribute(k, attrs[k]);
    if (text != null) e.textContent = text;
    return e;
  }

  function open(o) {
    close();
    if (!document.getElementById(ID + '_css')) { var s = el('style', { id: ID + '_css' }); s.textContent = css; document.head.appendChild(s); }
    var root = el('div', { id: ID, role: 'dialog', 'aria-modal': 'true', 'aria-label': 'MyOpenMath helper' });
    var bar1 = el('div', { class: 'bar' });
    var title = el('b', {}, 'MyOpenMath helper');
    var pill = el('span', { class: 'pill nobody' }, 'Connecting…');
    var ctl = el('button', { type: 'button' }, 'Take control');
    var status = el('span', { class: 'status grow' }, 'Connecting to the helper…');
    var closeBtn = el('button', { type: 'button' }, 'Close');
    [title, pill, ctl, status, closeBtn].forEach(function (x) { bar1.appendChild(x); });
    var bar2 = el('div', { class: 'bar' });
    var course = el('select', { 'aria-label': 'Course' }); course.appendChild(el('option', { value: '' }, 'Course: loading…'));
    var assess = el('select', { 'aria-label': 'Assessment' }); assess.appendChild(el('option', { value: '' }, 'Assessment'));
    var preview = el('button', { type: 'button' }, 'Preview in MyOpenMath');
    var add = el('button', { type: 'button', class: 'primary' }, 'Publish to selected assessment');
    preview.disabled = add.disabled = true;
    var working = false;
    var attempted = new Set();
    var pendingAttachment = new Set();
    var result = el('span', { class: 'grow' });
    [course, assess, preview, add, result].forEach(function (x) { bar2.appendChild(x); });
    var view = el('div', { class: 'view' });
    var canvas = el('canvas', { tabindex: '0', 'aria-label': 'The MyOpenMath page' });
    var keys = el('textarea', { class: 'keys', 'aria-hidden': 'true', autocomplete: 'off', autocapitalize: 'off', spellcheck: 'false' });
    view.appendChild(canvas); view.appendChild(keys);
    root.appendChild(bar1); root.appendChild(bar2); root.appendChild(view);
    document.body.appendChild(root);

    var ws = null, pageW = 1, pageH = 1, scale = 1, controller = 'nobody', lastMove = 0;
    var ctx2d = canvas.getContext('2d');
    function send(v) { if (ws && ws.readyState === 1) ws.send(JSON.stringify(v)); }
    function setControl(c) {
      controller = c;
      pill.className = 'pill ' + c;
      pill.textContent = c === 'you' ? 'You have control' : c === 'helper' ? 'The helper is working' : 'Nobody is driving';
      ctl.textContent = c === 'you' ? 'Release control' : 'Take control';
      if (c === 'you') canvas.focus();
    }
    function fill(sel, list, label) {
      sel.innerHTML = '';
      sel.appendChild(el('option', { value: '' }, list.length ? label : label + ' (none found)'));
      list.forEach(function (x) { sel.appendChild(el('option', { value: String(x.id) }, x.name)); });
    }
    function targetKey() { return course.value + ':' + assess.value; }
    function ready() {
      course.disabled = assess.disabled = working;
      preview.disabled = working || !(course.value && assess.value);
      add.disabled = preview.disabled || attempted.has(targetKey());
      add.textContent = pendingAttachment.has(targetKey()) ? 'Retry attachment' : 'Publish to selected assessment';
    }

    ctl.onclick = function () { send({ type: controller === 'you' ? 'release' : 'take' }); };
    closeBtn.onclick = close;
    course.onchange = function () { fill(assess, [], 'Assessment'); assess.firstChild.textContent = 'Assessment: loading…'; ready(); if (course.value) send({ type: 'assessments', cid: +course.value }); };
    assess.onchange = ready;
    function openEditor(publish) {
      result.textContent = ''; result.className = 'grow';
      if (publish) attempted.add(targetKey());
      working = true;
      ready();
      send({ type: 'add', cid: +course.value, aid: +assess.value, question: o.question, save: publish });
    }
    preview.onclick = function () { openEditor(false); };
    add.onclick = function () { openEditor(true); };

    // The screen's real pixels per CSS pixel: the page is rendered and drawn at this density, so
    // text stays sharp on high-density screens.
    var dpr = Math.min(Math.max(window.devicePixelRatio || 1, 1), 3);
    var url = String(o.server).replace(/\/$/, '') + '/browser?token=' + encodeURIComponent(o.token || '');
    try { ws = new WebSocket(url); } catch (e) { status.textContent = 'Could not connect: ' + e.message; status.className = 'bad grow'; return; }
    ws.binaryType = 'arraybuffer';
    ws.onopen = function () {
      // Measured after layout, so the page is exactly the panel's size and is never rescaled.
      requestAnimationFrame(function () {
        var size = view.getBoundingClientRect();
        send({ type: 'hello', width: Math.floor(size.width) - 8, height: Math.floor(size.height) - 8, dpr: dpr });
        send({ type: 'courses' });
      });
    };
    ws.onclose = function () { status.textContent = 'The helper connection closed.'; status.className = 'bad grow'; };
    ws.onerror = function () { status.textContent = 'Could not reach the helper at ' + o.server + '. Check the address and access code.'; status.className = 'bad grow'; };
    ws.onmessage = function (ev) {
      if (typeof ev.data !== 'string') {
        var dv = new DataView(ev.data);
        pageW = dv.getFloat32(0, true) || pageW; pageH = dv.getFloat32(4, true) || pageH;
        var blob = new Blob([new Uint8Array(ev.data, 8)], { type: 'image/jpeg' });
        createImageBitmap(blob).then(function (img) {
          var box = view.getBoundingClientRect();
          // Never enlarge: 1 CSS pixel of page = 1 CSS pixel of panel unless the panel is smaller.
          scale = Math.min((box.width - 8) / pageW, (box.height - 8) / pageH, 1);
          var w = pageW * scale, h = pageH * scale;
          var bw = Math.round(w * dpr), bh = Math.round(h * dpr);
          if (canvas.width !== bw || canvas.height !== bh) { canvas.width = bw; canvas.height = bh; }
          canvas.style.width = w + 'px'; canvas.style.height = h + 'px';
          canvas.style.left = Math.round((box.width - w) / 2) + 'px'; canvas.style.top = '4px';
          ctx2d.imageSmoothingQuality = 'high';
          ctx2d.drawImage(img, 0, 0, bw, bh);
          send({ type: 'ack' });
        }, function () { send({ type: 'ack' }); });
        return;
      }
      var m = JSON.parse(ev.data);
      if (m.type === 'control') setControl(m.controller);
      else if (m.type === 'status') { status.textContent = m.line; status.className = 'status grow'; }
      else if (m.type === 'error') { working = false; ready(); status.textContent = m.message; status.className = 'bad grow'; }
      else if (m.type === 'courses') {
        fill(course, m.list, 'Pick a course');
        if (o.course && m.list.some(function (c) { return c.id === o.course; })) { course.value = String(o.course); course.onchange(); }
      } else if (m.type === 'assessments' && String(m.cid) === course.value) {
        fill(assess, m.list, 'Pick an assessment');
        if (o.assessment && m.list.some(function (a) { return a.id === o.assessment; })) assess.value = String(o.assessment);
        ready();
        if (o.onTarget) o.onTarget(+course.value, +assess.value || 0);
      } else if (m.type === 'added') {
        if (m.retryAttachment) {
          attempted.delete(targetKey());
          pendingAttachment.add(targetKey());
        } else if (m.ok && attempted.has(targetKey())) {
          pendingAttachment.delete(targetKey());
        }
        working = false;
        ready();
        result.textContent = (m.ok ? '✔ ' : '') + m.message;
        result.className = (m.ok ? 'good' : 'bad') + ' grow';
      }
    };

    // ---- the teacher's input, in page pixels ----
    function at(e) { var r = canvas.getBoundingClientRect(); return { x: (e.clientX - r.left) / scale, y: (e.clientY - r.top) / scale }; }
    function btn(e) { return e.button === 2 ? 'right' : e.button === 1 ? 'middle' : 'left'; }
    canvas.addEventListener('mousedown', function (e) { var p = at(e); keys.focus(); send({ type: 'mouse', kind: 'down', x: p.x, y: p.y, button: btn(e), clicks: e.detail || 1 }); e.preventDefault(); });
    canvas.addEventListener('mouseup', function (e) { var p = at(e); send({ type: 'mouse', kind: 'up', x: p.x, y: p.y, button: btn(e), clicks: e.detail || 1 }); });
    canvas.addEventListener('mousemove', function (e) {
      if (controller !== 'you') return;
      var now = Date.now(); if (now - lastMove < 33) return; lastMove = now;
      var p = at(e); send({ type: 'mouse', kind: 'move', x: p.x, y: p.y });
    });
    canvas.addEventListener('wheel', function (e) { var p = at(e); send({ type: 'wheel', x: p.x, y: p.y, dx: e.deltaX, dy: e.deltaY }); e.preventDefault(); }, { passive: false });
    canvas.addEventListener('contextmenu', function (e) { e.preventDefault(); });
    var special = /^(Enter|Backspace|Tab|Escape|Delete|ArrowLeft|ArrowUp|ArrowRight|ArrowDown|Home|End|PageUp|PageDown)$/;
    function mods(e) { return (e.altKey ? 1 : 0) | (e.ctrlKey ? 2 : 0) | (e.metaKey ? 4 : 0) | (e.shiftKey ? 8 : 0); }
    keys.addEventListener('keydown', function (e) {
      if (special.test(e.key)) { send({ type: 'key', key: e.key, down: true, modifiers: mods(e) }); e.preventDefault(); }
      else if ((e.ctrlKey || e.metaKey) && /^[acxyz]$/i.test(e.key)) { send({ type: 'key', key: e.key, down: true, modifiers: mods(e) }); }
    });
    keys.addEventListener('keyup', function (e) { if (special.test(e.key)) { send({ type: 'key', key: e.key, down: false, modifiers: mods(e) }); e.preventDefault(); } });
    keys.addEventListener('input', function () { if (keys.value) { send({ type: 'text', text: keys.value }); keys.value = ''; } });
    keys.addEventListener('paste', function (e) { var t = (e.clipboardData || window.clipboardData).getData('text'); if (t) send({ type: 'text', text: t }); e.preventDefault(); });
    root.addEventListener('keydown', function (e) { if (e.key === 'Escape' && document.activeElement !== keys) close(); });
    root._ws = ws;
  }

  function close() {
    var r = document.getElementById(ID);
    if (r) { try { r._ws && r._ws.close(); } catch (e) {} r.remove(); }
  }

  window.dadBrowser = { open: open, close: close };
})();
