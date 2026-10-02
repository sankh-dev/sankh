import format from '../../../../docs/format.md?raw';
import { renderMarkdown, type RenderedDoc } from './markdown.ts';

const local = import.meta.glob<string>('/src/content/*.md', {
	query: '?raw',
	import: 'default',
	eager: true
});

const sources: Record<string, string> = { format };
for (const [path, source] of Object.entries(local)) {
	const slug = path.split('/').pop()!.replace(/\.md$/, '');
	sources[slug] = source;
}

export async function loadDoc(slug: string): Promise<RenderedDoc | undefined> {
	const source = sources[slug];
	return source === undefined ? undefined : renderMarkdown(source);
}
