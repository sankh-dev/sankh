<script lang="ts">
	import { untrack } from 'svelte';
	import { EditorView, basicSetup } from 'codemirror';
	import {
		Decoration,
		ViewPlugin,
		hoverTooltip,
		keymap,
		placeholder as placeholderText,
		type DecorationSet,
		type ViewUpdate
	} from '@codemirror/view';
	import { StateEffect } from '@codemirror/state';
	import { indentWithTab } from '@codemirror/commands';
	import { autocompletion } from '@codemirror/autocomplete';
	import {
		HighlightStyle,
		StreamLanguage,
		foldAll as foldAllCmd,
		syntaxHighlighting,
		unfoldAll as unfoldAllCmd
	} from '@codemirror/language';
	import { shell } from '@codemirror/legacy-modes/mode/shell';
	import { json } from '@codemirror/lang-json';
	import { tags } from '@lezer/highlight';
	import { sankhCompletions } from './completions';
	import { needsValue, refSpans, varTipDom, type VarInfo, type VarOption, type VarStatus } from './vars';

	interface Props {
		value: string;
		vars?: VarOption[];
		/** Flags `$VAR` references that still need a value. */
		varStatus?: (name: string) => VarStatus;
		/** Value and source shown when hovering any `$VAR` reference. */
		varInfo?: (name: string) => VarInfo;
		/** Selected environment, named in the hover tooltip. */
		env?: string;
		onsetvar?: (name: string, anchor: DOMRect) => void;
		onchange?: (value: string) => void;
		/** Fixed when the editor is created. */
		language?: 'shell' | 'json';
		placeholder?: string;
	}

	let {
		value = $bindable(),
		vars = [],
		varStatus,
		varInfo,
		env = '',
		onsetvar,
		onchange,
		language = 'shell',
		placeholder = ''
	}: Props = $props();
	let view: EditorView | undefined;

	export function foldAll() {
		if (view) foldAllCmd(view);
	}

	export function unfoldAll() {
		if (view) unfoldAllCmd(view);
	}

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
			}
		},
		{ dark: true }
	);

	/** Same colours as the response JSON tree; the default style is for light themes. */
	const jsonColors = syntaxHighlighting(
		HighlightStyle.define([
			{ tag: tags.propertyName, color: 'var(--info)' },
			{ tag: tags.string, color: 'var(--ok)' },
			{ tag: tags.number, color: 'var(--warn)' },
			{ tag: [tags.bool, tags.null], color: 'var(--accent)' },
			{ tag: [tags.brace, tags.squareBracket, tags.separator], color: 'var(--muted)' }
		])
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
		const hit = refSpans(v.state.doc.toString()).find((r) => pos >= r.from && pos <= r.to);
		if (!hit) return null;
		const info: VarInfo = varInfo?.(hit.name) ?? {
			name: hit.name,
			value: null,
			source: null,
			status: varStatus?.(hit.name) ?? 'ok'
		};
		if (!varInfo && !needsValue(info.status)) return null;
		return {
			pos: hit.from,
			end: hit.to,
			above: true,
			create() {
				const edit = onsetvar ? (anchor: DOMRect) => onsetvar?.(hit.name, anchor) : undefined;
				return { dom: varTipDom(info, env, edit) };
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
				untrack(() => (language === 'json' ? [json(), jsonColors] : StreamLanguage.define(shell))),
				placeholderText(untrack(() => placeholder)),
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
