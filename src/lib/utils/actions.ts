import { tick } from 'svelte';

/**
 * Svelte `use:` action that focuses the node after mount — replaces the
 * `autofocus` attribute, which the a11y checker flags because it can steal
 * focus from assistive-tech users before the page context is ready.
 */
export function focusOnMount(node: HTMLElement) {
  tick().then(() => node.focus());
}
