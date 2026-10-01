import type { Completion, CompletionContext, CompletionResult } from '@codemirror/autocomplete';

const ANNOTATIONS: Completion[] = [
	{ label: '@name', detail: 'display name', apply: '@name ' },
	{ label: '@description', detail: 'notes', apply: '@description ' },
	{ label: '@tags', detail: 'space-separated labels', apply: '@tags ' },
	{ label: '@expect status', detail: '200 | 2xx | 200|201', apply: '@expect status ' },
	{ label: '@expect json', detail: '<jq> <op> <value>', apply: '@expect json .' },
	{ label: '@capture', detail: 'NAME=<jq> | header X | status', apply: '@capture ' }
];

const OPERATORS: Completion[] = ['==', '!=', '>', '>=', '<', '<=', 'exists', 'matches', 'contains'].map(
	(op) => ({ label: op, type: 'keyword' })
);

/** Builds a completion source; `vars` is read lazily so it stays current. */
export function sankhCompletions(vars: () => string[]) {
	return (ctx: CompletionContext): CompletionResult | null => {
		const line = ctx.state.doc.lineAt(ctx.pos);
		const before = line.text.slice(0, ctx.pos - line.from);

		const variable = /\$\{?([A-Za-z_][A-Za-z0-9_]*)?$/.exec(before);
		if (variable) {
			const start = ctx.pos - (variable[1]?.length ?? 0);
			return {
				from: start,
				options: vars().map((v) => ({ label: v, type: 'variable' })),
				validFor: /^[A-Za-z0-9_]*$/
			};
		}

		if (/^\s*#/.test(before)) {
			const ann = /@[\w ]*$/.exec(before);
			if (ann && !/@expect json .+ /.test(before)) {
				return { from: ctx.pos - ann[0].length, options: ANNOTATIONS };
			}
			if (/@expect json \S+ \w*$/.test(before)) {
				const word = /\w*$/.exec(before)![0];
				return { from: ctx.pos - word.length, options: OPERATORS };
			}
		}
		return null;
	};
}
