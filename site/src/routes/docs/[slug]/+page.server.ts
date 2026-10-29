import { error, redirect } from '@sveltejs/kit';
import { docPages } from '#lib/docs.ts';
import { loadDoc } from '#lib/server/content.ts';
import type { EntryGenerator, PageServerLoad } from './$types';

const moved: Record<string, string> = { 'trust-secrets': 'secrets-safety' };

export const entries: EntryGenerator = () =>
	[...docPages.map(({ slug }) => slug), ...Object.keys(moved)].map((slug) => ({ slug }));

export const load: PageServerLoad = async ({ params }) => {
	if (moved[params.slug]) redirect(308, `/docs/${moved[params.slug]}`);

	const meta = docPages.find((d) => d.slug === params.slug);
	const doc = meta && (await loadDoc(params.slug));
	if (!meta || !doc) error(404, 'Not found');

	const index = docPages.indexOf(meta);
	return {
		...doc,
		slug: meta.slug,
		description: meta.description,
		prev: docPages[index - 1],
		next: docPages[index + 1]
	};
};
