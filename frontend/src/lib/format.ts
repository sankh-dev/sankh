export function formatSize(n: number) {
	if (n < 1024) return `${n} B`;
	if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
	return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

/**
 * Pretty-prints a JSON body. Bare variable references outside strings
 * (`{"id": $ID}`) are not JSON, so they are swapped for placeholder strings
 * while parsing and restored afterwards. Throws if the body is not JSON.
 */
export function formatJson(text: string): string {
	const vars: string[] = [];
	let out = '';
	let inString = false;
	for (let i = 0; i < text.length; i++) {
		const c = text[i];
		if (inString) {
			out += c;
			if (c === '\\') out += text[++i] ?? '';
			else if (c === '"') inString = false;
			continue;
		}
		if (c === '"') {
			inString = true;
			out += c;
			continue;
		}
		const ref = c === '$' && /^\$(\{[A-Za-z_][A-Za-z0-9_]*\}|[A-Za-z_][A-Za-z0-9_]*)/.exec(text.slice(i));
		if (ref) {
			out += `"\\u0000${vars.length}\\u0000"`;
			vars.push(ref[0]);
			i += ref[0].length - 1;
			continue;
		}
		out += c;
	}
	const pretty = JSON.stringify(JSON.parse(out), null, 2);
	return pretty.replace(/"\\u0000(\d+)\\u0000"/g, (_, n) => vars[Number(n)]);
}
