<!-- Spec: docs/design/components/theme-switch.md -->
<script lang="ts">
	import { theme, type ThemePreference } from '../../design/theme.svelte';
	import Select from '../Select/Select.svelte';

	const options: readonly { value: ThemePreference; label: string }[] = [
		{ value: 'system', label: 'System' },
		{ value: 'dark', label: 'Dark' },
		{ value: 'light', label: 'Light' }
	];
</script>

<Select
	label="Theme"
	{options}
	bind:value={
		() => theme?.preference ?? 'system',
		(v: ThemePreference) => {
			theme?.set(v);
		}
	}
/>
