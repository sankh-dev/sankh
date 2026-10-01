<script lang="ts">
	import { untrack } from 'svelte';
	import { EditorView, basicSetup } from 'codemirror';
	import { keymap } from '@codemirror/view';
	import { indentWithTab } from '@codemirror/commands';
	import { autocompletion } from '@codemirror/autocomplete';
	import { StreamLanguage } from '@codemirror/language';
	import { shell } from '@codemirror/legacy-modes/mode/shell';
	import { sankhCompletions } from './completions';

	interface Props {
		value: string;
		vars?: string[];
		onchange?: (value: string) => void;
	}

	let { value = $bindable(), vars = [], onchange }: Props = $props();
	let view: EditorView | undefined;

	const theme = EditorView.theme(
		{
			'&': { height: '100%', backgroundColor: 'var(--bg)', color: 'var(--text)' },
			'.cm-scroller': { fontFamily: 'var(--mono)', fontSize: '12.5px' },
			'.cm-gutters': { backgroundColor: 'var(--panel)', color: 'var(--muted)', border: 'none' },
			'.cm-activeLine': { backgroundColor: '#ffffff08' },
			'.cm-activeLineGutter': { backgroundColor: '#ffffff0c' },
			'&.cm-focused .cm-cursor': { borderLeftColor: 'var(--accent-soft)' },
			'.cm-selectionBackground, &.cm-focused .cm-selectionBackground': {
				backgroundColor: '#1d9a7855 !important'
			},
			'.cm-tooltip': { backgroundColor: 'var(--panel-2)', border: '1px solid var(--border)' }
		},
		{ dark: true }
	);

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
</script>

<div class="editor" {@attach editor}></div>

<style>
	.editor {
		height: 100%;
		overflow: hidden;
	}
</style>
