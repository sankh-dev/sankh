export type BodyKind = 'image' | 'html' | 'xml' | 'json' | 'text';

/** The media type of `Content-Type`, lowercased and without parameters. */
export function mediaType(headers: [string, string][]): string {
	let value = '';
	for (const [k, v] of headers) if (k.toLowerCase() === 'content-type') value = v;
	return value.split(';')[0].trim().toLowerCase();
}

export function bodyKind(headers: [string, string][], body: string): BodyKind {
	const mime = mediaType(headers);
	if (mime.startsWith('image/')) return 'image';
	if (mime === 'text/html' || mime === 'application/xhtml+xml') return 'html';
	if (mime.endsWith('/xml') || mime.endsWith('+xml')) return 'xml';

	const start = body.trimStart().slice(0, 64).toLowerCase();
	if (!mime || mime === 'text/plain' || mime === 'application/octet-stream') {
		if (start.startsWith('<?xml')) return 'xml';
		if (start.startsWith('<!doctype html') || start.startsWith('<html')) return 'html';
	}
	return prettyJson(body) === null ? 'text' : 'json';
}

export function prettyJson(text: string): string | null {
	try {
		return JSON.stringify(JSON.parse(text), null, 2);
	} catch {
		return null;
	}
}

function escapeText(s: string) {
	return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function escapeAttr(s: string) {
	return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/"/g, '&quot;');
}

function serialize(node: Node, depth: number, out: string[]) {
	const pad = '  '.repeat(depth);
	switch (node.nodeType) {
		case Node.ELEMENT_NODE: {
			const el = node as Element;
			const attrs = Array.from(el.attributes, (a) => ` ${a.name}="${escapeAttr(a.value)}"`).join('');
			const children = Array.from(el.childNodes).filter(
				(c) => c.nodeType !== Node.TEXT_NODE || c.textContent?.trim()
			);
			if (children.length === 0) {
				out.push(`${pad}<${el.tagName}${attrs}/>`);
			} else if (children.length === 1 && children[0].nodeType === Node.TEXT_NODE) {
				const text = escapeText(children[0].textContent!.trim());
				out.push(`${pad}<${el.tagName}${attrs}>${text}</${el.tagName}>`);
			} else {
				out.push(`${pad}<${el.tagName}${attrs}>`);
				for (const c of children) serialize(c, depth + 1, out);
				out.push(`${pad}</${el.tagName}>`);
			}
			break;
		}
		case Node.TEXT_NODE:
			out.push(pad + escapeText(node.textContent!.trim()));
			break;
		case Node.CDATA_SECTION_NODE:
			out.push(`${pad}<![CDATA[${node.textContent}]]>`);
			break;
		case Node.COMMENT_NODE:
			out.push(`${pad}<!--${node.textContent}-->`);
			break;
		case Node.PROCESSING_INSTRUCTION_NODE: {
			const pi = node as ProcessingInstruction;
			out.push(`${pad}<?${pi.target} ${pi.data}?>`);
			break;
		}
		case Node.DOCUMENT_TYPE_NODE: {
			const dt = node as DocumentType;
			const id = dt.publicId
				? ` PUBLIC "${dt.publicId}" "${dt.systemId}"`
				: dt.systemId
					? ` SYSTEM "${dt.systemId}"`
					: '';
			out.push(`${pad}<!DOCTYPE ${dt.name}${id}>`);
			break;
		}
	}
}

/** Indents XML two spaces per level; `null` if it does not parse. */
export function formatXml(text: string): string | null {
	const doc = new DOMParser().parseFromString(text, 'application/xml');
	if (doc.getElementsByTagName('parsererror').length) return null;
	const out: string[] = [];
	const decl = text.trimStart().match(/^<\?xml[^?]*\?>/);
	if (decl) out.push(decl[0]);
	for (const c of Array.from(doc.childNodes)) serialize(c, 0, out);
	return out.join('\n');
}
