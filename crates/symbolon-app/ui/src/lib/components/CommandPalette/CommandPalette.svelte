<!-- Spec: docs/design/components/command-palette.md -->
<script lang="ts">
	import { Command, Dialog } from 'bits-ui';
	import Kbd from '../Kbd/Kbd.svelte';
	import { formatKey } from '../../keyboard/keymap';

	/** An action offered by the palette. */
	export interface PaletteCommand {
		id: string;
		label: string;
		group: string;
		/** Keymap notation, e.g. "g l"; shown as a hint. */
		shortcut?: string;
		run: () => void;
	}

	interface Props {
		open?: boolean;
		commands: readonly PaletteCommand[];
	}

	let { open = $bindable(false), commands }: Props = $props();
	const groups = $derived([...new Set(commands.map((c) => c.group))]);

	function run(command: PaletteCommand) {
		open = false;
		command.run();
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Portal>
		<Dialog.Overlay class="sy-scrim" />
		<Dialog.Content class="sy-palette" data-sy-text-entry="">
			<Dialog.Title class="sy-visually-hidden">Command palette</Dialog.Title>
			<Command.Root label="Commands" loop>
				<Command.Input class="sy-palette-input" placeholder="Type a command" />
				<Command.List class="sy-palette-list">
					<Command.Viewport>
						<Command.Empty class="sy-palette-empty">No matching commands</Command.Empty>
						{#each groups as group (group)}
							<Command.Group value={group}>
								<Command.GroupHeading class="sy-palette-heading">{group}</Command.GroupHeading>
								<Command.GroupItems>
									{#each commands.filter((c) => c.group === group) as command (command.id)}
										<Command.Item
											class="sy-palette-item"
											value={command.id}
											keywords={[command.label]}
											onSelect={() => {
												run(command);
											}}
										>
											<span>{command.label}</span>
											{#if command.shortcut}<Kbd keys={formatKey(command.shortcut)} />{/if}
										</Command.Item>
									{/each}
								</Command.GroupItems>
							</Command.Group>
						{/each}
					</Command.Viewport>
				</Command.List>
			</Command.Root>
		</Dialog.Content>
	</Dialog.Portal>
</Dialog.Root>

<style>
	:global(.sy-palette) {
		position: fixed;
		top: 15vh;
		left: 50%;
		z-index: 50;
		width: min(90vw, calc(var(--sy-control-height-md) * 18));
		overflow: hidden;
		border: var(--sy-border-width-default) solid var(--sy-color-border-default);
		border-radius: var(--sy-panel-radius);
		background: var(--sy-color-surface-raised);
		color: var(--sy-color-text-primary);
		box-shadow: var(--sy-shadow-overlay);
		transform: translateX(-50%);
	}

	:global(.sy-palette-input) {
		width: 100%;
		height: var(--sy-toolbar-height);
		padding: 0 var(--sy-space-16);
		border: 0;
		border-bottom: var(--sy-border-width-default) solid var(--sy-color-border-default);
		background: transparent;
		color: var(--sy-color-text-primary);
		font: inherit;
		font-size: var(--sy-font-size-ui);
		outline: none;
	}

	:global(.sy-palette-list) {
		max-height: 50vh;
		overflow: auto;
		padding: var(--sy-space-4);
	}

	:global(.sy-palette-heading) {
		padding: var(--sy-space-8) var(--sy-space-8) var(--sy-space-4);
		color: var(--sy-color-text-muted);
		font-size: var(--sy-font-size-small);
	}

	/* Bits marks the active command with data-selected. */
	:global(.sy-palette-item) {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: var(--sy-control-height-md);
		padding: 0 var(--sy-space-8);
		border-left: var(--sy-border-width-strong) solid transparent;
		border-radius: var(--sy-radius-sm);
		cursor: pointer;
	}

	:global(.sy-palette-item[data-selected]) {
		border-left-color: var(--sy-color-accent-default);
		background: var(--sy-color-surface-hover);
	}

	:global(.sy-palette-empty) {
		padding: var(--sy-space-16);
		color: var(--sy-color-text-muted);
		text-align: center;
	}

	@media (pointer: coarse) {
		:global(.sy-palette-item) {
			min-height: var(--sy-control-height-touch);
		}
	}
</style>
