import { SvelteSet } from 'svelte/reactivity';
import type { TreeNode } from './types';

const KEY = 'sankh-folders';
/** Older per-collection key for a collapsed collection section. */
const LEGACY = 'sankh-collapsed:';

function restore(): string[] {
	let out: string[] = [];
	try {
		const saved = JSON.parse(localStorage.getItem(KEY) ?? '[]');
		if (Array.isArray(saved)) out = saved.filter((k) => typeof k === 'string');
	} catch {
		// start with everything open
	}
	for (const k of Object.keys(localStorage)) {
		if (!k.startsWith(LEGACY)) continue;
		out.push(`${k.slice(LEGACY.length)}:`);
		localStorage.removeItem(k);
	}
	return out;
}

/**
 * Collapsed sidebar entries as `cid:path`; every other one is open.
 * The empty path is the collection section itself.
 */
const collapsed = new SvelteSet<string>(restore());

const key = (cid: string, path: string) => `${cid}:${path}`;

function save() {
	localStorage.setItem(KEY, JSON.stringify([...collapsed]));
}

function folderPaths(node: TreeNode, out: string[] = []) {
	if (node.type === 'folder') {
		if (node.path) out.push(node.path);
		for (const child of node.children) folderPaths(child, out);
	}
	return out;
}

export function isOpen(cid: string, path: string) {
	return !collapsed.has(key(cid, path));
}

export function toggle(cid: string, path: string) {
	const k = key(cid, path);
	if (collapsed.has(k)) collapsed.delete(k);
	else collapsed.add(k);
	save();
}

/** Collapses every folder of the collection's tree; with `section`, the collection too. */
export function collapseAll(cid: string, tree: TreeNode | null, section = false) {
	if (tree) for (const path of folderPaths(tree)) collapsed.add(key(cid, path));
	if (section) collapsed.add(key(cid, ''));
	save();
}

/** Opens the collection and every folder in it. */
export function expandAll(cid: string) {
	const prefix = `${cid}:`;
	for (const k of [...collapsed]) if (k.startsWith(prefix)) collapsed.delete(k);
	save();
}

/** Opens the collection and the folders containing `path`, so a selected request is visible. */
export function reveal(cid: string, path: string) {
	const parts = path.split('/');
	let changed = collapsed.delete(key(cid, ''));
	for (let i = 1; i < parts.length; i++) {
		changed = collapsed.delete(key(cid, parts.slice(0, i).join('/'))) || changed;
	}
	if (changed) save();
}

/** Whether any of these collections, or a folder in them, is collapsed. */
export function anyCollapsed(cids: string[]) {
	for (const k of collapsed) if (cids.some((cid) => k.startsWith(`${cid}:`))) return true;
	return false;
}

/** Whether the collection has folders below its top level. */
export function hasFolders(tree: TreeNode | null) {
	return !!tree && folderPaths(tree).length > 0;
}
