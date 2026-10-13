import type { Attachment } from 'svelte/attachments';
import type { VarOption } from './vars';

type Field = HTMLInputElement | HTMLTextAreaElement;

/** `$NAME` or `${NAME` right before the caret, not escaped as `\$`. */
const TRIGGER = /(^|[^\\])\$(\{?)([A-Za-z_][A-Za-z0-9_]*)?$/;
const LIMIT = 40;

const MIRRORED = [
	'boxSizing',
	'width',
	'paddingTop',
	'paddingRight',
	'paddingBottom',
	'paddingLeft',
	'borderTopWidth',
	'borderLeftWidth',
	'fontFamily',
	'fontSize',
	'fontWeight',
	'lineHeight',
	'letterSpacing',
	'tabSize'
] as const;

/** Viewport position of the caret, measured with an off-screen copy of the field. */
function caretPoint(el: Field, pos: number) {
	const style = getComputedStyle(el);
	const mirror = document.createElement('div');
	for (const k of MIRRORED) mirror.style[k] = style[k];
	mirror.style.position = 'fixed';
	mirror.style.visibility = 'hidden';
	mirror.style.top = '0';
	mirror.style.left = '-9999px';
	mirror.style.whiteSpace = el instanceof HTMLTextAreaElement ? 'pre-wrap' : 'pre';
	mirror.style.overflowWrap = 'break-word';
	mirror.textContent = el.value.slice(0, pos);
	const mark = document.createElement('span');
	mark.textContent = '\u200b';
	mirror.append(mark);
	document.body.append(mirror);
	const box = el.getBoundingClientRect();
	const x = box.left + mark.offsetLeft - el.scrollLeft;
	const y = box.top + mark.offsetTop - el.scrollTop + mark.offsetHeight;
	mirror.remove();
	return {
		x: Math.min(Math.max(box.left, x), box.right - 8),
		y: el instanceof HTMLTextAreaElement ? Math.min(y, box.bottom) : box.bottom
	};
}

function rank(options: VarOption[], prefix: string) {
	const p = prefix.toLowerCase();
	if (!p) return options.slice(0, LIMIT);
	const starts = options.filter((o) => o.name.toLowerCase().startsWith(p));
	const contains = options.filter((o) => !o.name.toLowerCase().startsWith(p) && o.name.toLowerCase().includes(p));
	return [...starts, ...contains].slice(0, LIMIT);
}

/**
 * Suggests environment variables after `$` or `${` in a text field.
 * Arrow keys move, Enter or Tab inserts, Escape closes.
 */
export function varSuggest(options: () => VarOption[]): Attachment<Field> {
	return (el) => {
		let list: HTMLDivElement | null = null;
		let items: VarOption[] = [];
		let active = 0;
		/** Where the typed name starts, and whether it was opened with `${`. */
		let from = 0;
		let braced = false;
		/** Our own `input` event after inserting a name; not a keystroke. */
		let inserting = false;

		function close() {
			list?.remove();
			list = null;
		}

		function render() {
			if (!list) return;
			list.replaceChildren(
				...items.map((o, i) => {
					const row = document.createElement('div');
					row.className = `var-suggest-item${i === active ? ' active' : ''}${o.status === 'ok' ? '' : ` ${o.status}`}`;
					row.setAttribute('role', 'option');
					row.setAttribute('aria-selected', String(i === active));
					const name = document.createElement('span');
					name.className = 'name';
					name.textContent = o.name;
					const detail = document.createElement('span');
					detail.className = 'detail';
					detail.textContent = o.detail;
					row.append(name, detail);
					row.onmousedown = (e) => {
						e.preventDefault();
						pick(i);
					};
					return row;
				})
			);
			list.children[active]?.scrollIntoView({ block: 'nearest' });
		}

		function open() {
			const p = caretPoint(el, from - (braced ? 2 : 1));
			if (!list) {
				list = document.createElement('div');
				list.className = 'var-suggest';
				list.setAttribute('role', 'listbox');
				list.setAttribute('aria-label', 'Variables');
				document.body.append(list);
			}
			const below = p.y + 4;
			const height = Math.min(items.length * 28 + 8, 260);
			list.style.left = `${Math.min(p.x, window.innerWidth - 340)}px`;
			list.style.top = `${below + height > window.innerHeight ? Math.max(4, p.y - height - el.offsetHeight) : below}px`;
			render();
		}

		function update() {
			const caret = el.selectionStart ?? el.value.length;
			if (caret !== el.selectionEnd) return close();
			const m = TRIGGER.exec(el.value.slice(0, caret));
			if (!m) return close();
			const prefix = m[3] ?? '';
			braced = m[2] === '{';
			from = caret - prefix.length;
			items = rank(options(), prefix);
			if (!items.length || (items.length === 1 && items[0].name === prefix && !braced)) return close();
			active = 0;
			open();
		}

		function pick(i: number) {
			const o = items[i];
			if (!o) return;
			const caret = el.selectionStart ?? el.value.length;
			let end = caret;
			while (end < el.value.length && /[A-Za-z0-9_]/.test(el.value[end])) end++;
			const suffix = braced && el.value[end] !== '}' ? '}' : '';
			const skip = braced && el.value[end] === '}' ? 1 : 0;
			el.value = el.value.slice(0, from) + o.name + suffix + el.value.slice(end);
			const pos = from + o.name.length + suffix.length + skip;
			el.setSelectionRange(pos, pos);
			close();
			inserting = true;
			el.dispatchEvent(new Event('input', { bubbles: true }));
			el.dispatchEvent(new Event('change', { bubbles: true }));
			inserting = false;
		}

		function onkeydown(e: KeyboardEvent) {
			if (!list) return;
			if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
				e.preventDefault();
				active = (active + (e.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length;
				render();
			} else if (e.key === 'Enter' || e.key === 'Tab') {
				e.preventDefault();
				e.stopPropagation();
				pick(active);
			} else if (e.key === 'Escape') {
				e.preventDefault();
				e.stopPropagation();
				close();
			} else if (e.key === 'ArrowLeft' || e.key === 'ArrowRight' || e.key === 'Home' || e.key === 'End') {
				close();
			}
		}

		function oninput() {
			if (!inserting) update();
		}

		function onscroll(e: Event) {
			if (list && !list.contains(e.target as Node)) close();
		}

		const node: HTMLElement = el;
		node.addEventListener('input', oninput);
		node.addEventListener('keydown', onkeydown);
		node.addEventListener('blur', close);
		document.addEventListener('scroll', onscroll, true);
		return () => {
			close();
			node.removeEventListener('input', oninput);
			node.removeEventListener('keydown', onkeydown);
			node.removeEventListener('blur', close);
			document.removeEventListener('scroll', onscroll, true);
		};
	};
}
