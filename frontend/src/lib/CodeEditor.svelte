<script lang="ts">
	import { untrack } from 'svelte';
	import { EditorView, basicSetup } from 'codemirror';
	import { Decoration, ViewPlugin, hoverTooltip, keymap, type DecorationSet, type ViewUpdate } from '@codemirror/view';
	import { StateEffect } from '@codemirror/state';
	import { indentWithTab } from '@codemirror/commands';
	import { autocompletion } from '@codemirror/autocomplete';
	import { StreamLanguage } from '@codemirror/language';
	import { shell } from '@codemirror/legacy-modes/mode/shell';
	import { sankhCompletions } from './completions';
	import { STATUS_TEXT, needsValue, refSpans, type VarOption, type VarStatus } from './vars';

	interface Props {
		value: string;
		vars?: VarOption[];
		/** Flags `$VAR` references that still need a value. */
		varStatus?: (name: string) => VarStatus;
		onsetvar?: (name: string, anchor: DOMRect) => void;
		onchange?: (value: string) => void;
	}

	let { value = $bindable(), vars = [], varStatus, onsetvar, onchange }: Props = $props();
	let view: EditorView | undefined;

	const recheck = StateEffect.define<null>();

	const theme = EditorView.theme(
		{
			'&': { height: '100%', backgroundColor: 'var(--bg)', color: 'var(--text)' },
			'.cm-scroller': { fontFamily: 'var(--mono)', fontSize: '12.5px' },
			'.cm-gutters': { backgroundColor: 'var(--panel)', color: 'var(--muted)', border: 'none' },
			'.cm-activeLine': { backgroundColor: '#ffffff08' },
			'.cm-activeLineGutter': { backgroundColor: '#ffffff0c' },
			'&.cm-focused .cm-cursor': { borderLeftColor: 'var(--accent-soft)' },
			'.cm-selectionBackground, &.cm-focused .cm-selectionBackground': {
				backgroundColor: '#22a88255 !important'
			},
			'.cm-tooltip': { backgroundColor: 'var(--panel-2)', border: '1px solid var(--border)' },
			'.cm-var-unset': {
				textDecoration: 'underline wavy var(--warn)',
				textUnderlineOffset: '3px',
				backgroundColor: '#f2c14e14'
			},
			'.cm-var-missing': {
				textDecoration: 'underline wavy var(--fail)',
				textUnderlineOffset: '3px',
				backgroundColor: '#ff6b6b14'
			},
			'.cm-var-tip': {
				display: 'flex',
				alignItems: 'center',
				gap: '8px',
				padding: '6px 8px',
				fontFamily: 'var(--sans)',
				fontSize: '12px'
			},
			'.cm-var-tip button': { minHeight: '22px', fontSize: '11px', padding: '0 8px' }
		},
		{ dark: true }
	);

	function flagged(doc: string) {
		if (!varStatus) return [];
		return refSpans(doc)
			.map((r) => ({ ...r, status: varStatus(r.name) }))
			.filter((r) => needsValue(r.status));
	}

	function marks(v: EditorView): DecorationSet {
		return Decoration.set(
			flagged(v.state.doc.toString()).map((r) =>
				Decoration.mark({ class: `cm-var-${r.status}` }).range(r.from, r.to)
			)
		);
	}

	const varMarks = ViewPlugin.fromClass(
		class {
			decorations: DecorationSet;
			constructor(v: EditorView) {
				this.decorations = marks(v);
			}
			update(u: ViewUpdate) {
				if (u.docChanged || u.transactions.some((t) => t.effects.some((e) => e.is(recheck)))) {
					this.decorations = marks(u.view);
				}
			}
		},
		{ decorations: (p) => p.decorations }
	);

	const varTips = hoverTooltip((v, pos) => {
		const hit = flagged(v.state.doc.toString()).find((r) => pos >= r.from && pos <= r.to);
		if (!hit) return null;
		return {
			pos: hit.from,
			end: hit.to,
			above: true,
			create() {
				const dom = document.createElement('div');
				dom.className = 'cm-var-tip';
				const label = document.createElement('span');
				label.textContent = `${hit.name}: ${STATUS_TEXT[hit.status]}`;
				dom.append(label);
				if (onsetvar) {
					const btn = document.createElement('button');
					btn.textContent = 'Set value…';
					btn.onclick = () => onsetvar?.(hit.name, btn.getBoundingClientRect());
					dom.append(btn);
				}
				return { dom };
			}
		};
	});

	function editor(node: HTMLElement) {
		view = new EditorView({
			parent: node,
			doc: untrack(() => value),
			extensions: [
				basicSetup,
				keymap.of([indentWithTab]),
				StreamLanguage.define(shell),
				autocompletion({ override: [sankhCompletions(() => vars)] }),
				theme,
				varMarks,
				varTips,
				EditorView.updateListener.of((u) => {
					if (u.docChanged) {
						value = u.state.doc.toString();
						onchange?.(value);
					}
				})
			]
		});
		return () => view?.destroy();
	}

	$effect(() => {
		const next = value;
		if (view && next !== view.state.doc.toString()) {
			view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: next } });
		}
	});

	$effect(() => {
		void varStatus;
		view?.dispatch({ effects: recheck.of(null) });
	});
</script>

<div class="editor" {@attach editor}></div>

<style>
	.editor {
		height: 100%;
		overflow: hidden;
	}
</style>
