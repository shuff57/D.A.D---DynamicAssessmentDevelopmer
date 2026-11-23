// Expose the local student player's text and common controls to D.A.D's native widgets.
async (previewUrl, edits, action) => {
    const player = document.getElementById('app');
    if (location.protocol !== 'file:' || location.href.split('#')[0] !== previewUrl || !player) {
        throw new Error('The local student preview is unavailable.');
    }
    const view = () => document.getElementById('dadsoln')
        || player.querySelector('[role=alertdialog][aria-modal=true][open]')
        || player;

    const controls = window.dadStudentTextControls;
    const root = view();
    const control = controls?.[action];
    if (action !== null && (!control || !root.contains(control) || control.disabled)) {
        throw new Error('The preview changed. Refresh the text view and try again.');
    }
    for (const { id, value } of edits) {
        const field = controls?.[id];
        if (!field || !root.contains(field) || field.disabled || field.readOnly) continue;
        const choice = ['checkbox', 'radio'].includes(field.type);
        if (choice) {
            if (field.checked === (value === 'true')) continue;
            field.checked = value === 'true';
        } else {
            if (field.value === value) continue;
            field.value = value;
        }
        field.dispatchEvent(new Event('input', { bubbles: true }));
        field.dispatchEvent(new Event('change', { bubbles: true }));
    }
    if (control) {
        let previous = root.innerText;
        let quiet = 0;
        let changed = false;
        control.click();
        for (let i = 0; i < 25 && quiet < 3; i++) {
            await new Promise(resolve => setTimeout(resolve, 100));
            const current = view().innerText;
            if (current !== previous) {
                changed = true;
                quiet = 0;
                previous = current;
            } else if (changed && !current.includes('Loading...')) {
                quiet++;
            }
        }
    }

    const items = [];
    const fields = [];
    const normalize = text => (text || '').replace(/\s+/g, ' ').trim();
    let pending = '';
    const flush = () => {
        const text = normalize(pending);
        if (text) items.push({ kind: 'text', text });
        pending = '';
    };
    const add = (node, item) => {
        flush();
        items.push({ id: fields.push(node) - 1, ...item });
    };
    const name = node => normalize(
        node.getAttribute('aria-label')
        || node.getAttribute('aria-labelledby')?.split(/\s+/).map(id => document.getElementById(id)?.textContent).join(' ')
        || Array.from(node.labels || [], label => label.textContent).join(' ')
        || node.getAttribute('title'));
    const speakMath = node => {
        if (node.nodeType === Node.TEXT_NODE) return normalize(node.textContent);
        if (node.nodeType !== Node.ELEMENT_NODE || ['annotation', 'annotation-xml'].includes(node.localName)) return '';
        const parts = Array.from(node.children, speakMath);
        if (!parts.length) return normalize(node.textContent);
        switch (node.localName) {
            case 'mfrac': return parts.length === 2 ? `start fraction ${parts[0]} over ${parts[1]} end fraction` : parts.join(' ');
            case 'msqrt': return `square root of ${parts.join(' ')} end root`;
            case 'mroot': return `root of order ${parts[1]} of ${parts[0]} end root`;
            case 'msup': return `${parts[0]} to the power ${parts[1]} end power`;
            case 'msub': return `${parts[0]} subscript ${parts[1]} end subscript`;
            case 'msubsup': return `${parts[0]} subscript ${parts[1]} end subscript to the power ${parts[2]} end power`;
            default: return parts.join(' ');
        }
    };
    const walk = node => {
        if (node.nodeType === Node.TEXT_NODE) {
            pending += node.textContent;
            return;
        }
        if (node.nodeType !== Node.ELEMENT_NODE || node.hidden || node.getAttribute('aria-hidden') === 'true'
            || node.matches('.sr-only, .sronly, #skipnav, [id^=qhelp]')) return;
        const tag = node.tagName.toLowerCase();
        if (['script', 'style', 'noscript', 'template'].includes(tag)) return;
        const css = getComputedStyle(node);
        if (css.display === 'none' || css.visibility === 'hidden') return;
        if (node.matches('.dad-graph')) {
            // Read the seed-matched description even before IMathAS finishes drawing the SVG.
            const description = normalize(node.querySelector('.dad-graph-description')?.textContent);
            if (description) {
                flush();
                items.push({ kind: 'text', text: description });
                return;
            }
        }
        if (tag === 'mjx-container' || tag === 'math') {
            const math = tag === 'math' ? node : node.querySelector('math');
            const text = name(node) || node.querySelector('[data-semantic-speech]')?.getAttribute('data-semantic-speech')
                || node.getAttribute('alttext') || (math && speakMath(math))
                || node.querySelector('annotation')?.textContent;
            pending += ' ' + (text || 'Math expression. See browser view.') + ' ';
            return;
        }
        if (['img', 'svg', 'canvas'].includes(tag)) {
            if (tag === 'img' && node.getAttribute('alt') === '') return;
            const alt = name(node) || node.getAttribute('alt') || node.querySelector('title')?.textContent;
            if (alt || ['img', 'canvas'].includes(tag) || css.display === 'block') {
                pending += ' ' + (alt || 'Diagram. See browser view.') + ' ';
            }
            return;
        }
        if (tag === 'button' || tag === 'a') {
            const copy = node.cloneNode(true);
            copy.querySelectorAll('.sr-only, .sronly, [aria-hidden=true]').forEach(hidden => hidden.remove());
            const label = normalize(copy.textContent) || name(node);
            const dismiss = /^(ok|close)$/i.test(label) && node.closest('#dadsoln, [role=alertdialog][aria-modal=true]');
            const studentAction = /^(start|submit question|show detailed solution|hide detailed solution|get a similar question)$/i.test(label);
            const writtenExample = /^written example$/i.test(label) && node.getAttribute('href') === '#';
            if (tag === 'a' && label === 'Watch the help video' && /^https?:$/.test(node.protocol)) {
                flush();
                items.push({ kind: 'video_link', url: node.href });
                return;
            }
            if (!node.disabled && (tag === 'button' && (dismiss || studentAction) || tag === 'a' && writtenExample)) {
                add(node, { kind: 'button', label });
            }
            return;
        }
        if (['input', 'textarea', 'select'].includes(tag)) {
            if (['hidden', 'button', 'reset', 'submit'].includes(node.type) || node.disabled || node.readOnly) return;
            const label = name(node) || `Answer ${fields.filter(field => field.tagName !== 'BUTTON').length + 1}`;
            if (tag === 'select' && !node.multiple) {
                add(node, { kind: 'select', label, value: node.value,
                    options: Array.from(node.options).filter(option => !option.disabled).map(option => [option.value, option.label]) });
            } else if (['radio', 'checkbox'].includes(node.type)) {
                add(node, { kind: 'choice', label, group: node.type === 'radio' ? node.name : '',
                    checked: node.checked });
            } else if (['text', 'number', 'search', 'textarea'].includes(node.type)) {
                add(node, { kind: 'field', label: /^Question \d+$/i.test(label) ? 'Answer for ' + label : label,
                    hint: node.getAttribute('data-entrytip') || '', value: node.value,
                    multiline: tag === 'textarea' });
            } else {
                flush();
                items.push({ kind: 'text', text: `${label}. Use the browser view for this answer control.` });
            }
            return;
        }
        const block = tag === 'br' || /^(block|flex|grid|table|list-item)/.test(css.display);
        if (block) flush();
        for (const child of node.childNodes) walk(child);
        if (block) flush();
    };
    walk(view());
    flush();
    window.dadStudentTextControls = fields;
    return items;
}
