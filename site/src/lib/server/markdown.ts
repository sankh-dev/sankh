import { Marked, type Tokens } from 'marked';
import { createHighlighter, type Highlighter } from 'shiki';

const THEME = 'vitesse-dark';
const LANGS = ['bash', 'shellscript', 'toml', 'yaml', 'json', 'ini', 'dotenv', 'powershell'];

let highlighter: Promise<Highlighter> | undefined;

function getHighlighter() {
	highlighter ??= createHighlighter({ themes: [THEME], langs: LANGS });
	return highlighter;
}

export async function highlight(code: string, lang = 'text'): Promise<string> {
	const hl = await getHighlighter();
	const known = hl.getLoadedLanguages().includes(lang);
	return hl.codeToHtml(code.replace(/\n$/, ''), { lang: known ? lang : 'text', theme: THEME });
}

export function slugify(text: string): string {
	return text
		.toLowerCase()
		.replace(/<[^>]+>/g, '')
		.replace(/&[a-z]+;|&#\d+;/g, '')
		.replace(/[^a-z0-9\s-]/g, '')
		.trim()
		.replace(/\s+/g, '-');
}

export interface TocEntry {
	id: string;
	text: string;
}

export interface RenderedDoc {
	title: string;
	html: string;
	toc: TocEntry[];
}

export async function renderMarkdown(source: string): Promise<RenderedDoc> {
	const hl = await getHighlighter();
	const loaded = hl.getLoadedLanguages();
	let title = '';
	const toc: TocEntry[] = [];

	const marked = new Marked({
		gfm: true,
		renderer: {
			code({ text, lang }: Tokens.Code) {
				const l = lang?.trim().split(/\s+/)[0] || 'text';
				return hl.codeToHtml(text, { lang: loaded.includes(l) ? l : 'text', theme: THEME });
			},
			heading({ tokens, depth }: Tokens.Heading) {
				const inner = this.parser.parseInline(tokens);
				if (depth === 1) {
					title ||= inner.replace(/<[^>]+>/g, '');
					return '';
				}
				const id = slugify(inner);
				if (depth === 2) toc.push({ id, text: inner.replace(/<[^>]+>/g, '') });
				return `<h${depth} id="${id}"><a class="anchor" href="#${id}" aria-hidden="true">#</a>${inner}</h${depth}>\n`;
			},
			link({ href, tokens }: Tokens.Link) {
				const inner = this.parser.parseInline(tokens);
				const doc = href.match(/^(?:\.\.?\/)*(?:docs\/)?([a-z0-9-]+)\.md(#.*)?$/);
				if (doc) return `<a href="/docs/${doc[1]}${doc[2] ?? ''}">${inner}</a>`;
				const external = /^https?:/.test(href);
				return `<a href="${href}"${external ? ' rel="external"' : ''}>${inner}</a>`;
			}
		}
	});

	const html = await marked.parse(source);
	return { title, html, toc };
}
