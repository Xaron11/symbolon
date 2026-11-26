import { render, screen } from '@testing-library/svelte';
import type { ChartConfiguration } from 'chart.js';
import { flushSync } from 'svelte';
import { describe, expect, it, vi } from 'vitest';
import { text } from '../../../test/snippet';
import { theme } from '../../design/theme.svelte';
import ChartFrame from './ChartFrame.svelte';

const charts = vi.hoisted(() => [] as { config: unknown; destroy: ReturnType<typeof vi.fn> }[]);
vi.mock('chart.js', () => {
	class Chart {
		static register = vi.fn();
		destroy = vi.fn();
		constructor(
			_canvas: unknown,
			public config: unknown
		) {
			charts.push(this);
		}
	}
	return {
		Chart,
		BarController: {},
		BarElement: {},
		CategoryScale: {},
		LinearScale: {},
		Tooltip: {},
		Legend: {}
	};
});

describe('ChartFrame', () => {
	it('draws with the current theme, redraws on theme change and cleans up', () => {
		charts.length = 0;
		theme?.set('light');
		const config = vi.fn((palette: Readonly<Record<string, string>>): ChartConfiguration => ({
			type: 'bar',
			data: {
				labels: ['a'],
				datasets: [{ data: [1], backgroundColor: palette['category.different.fg'] ?? '' }]
			}
		}));
		const { unmount } = render(ChartFrame, { config, label: 'Files by size', table: text('a: 1') });
		flushSync();
		expect(charts).toHaveLength(1);
		const light = config.mock.calls.at(-1)?.[0]['color.surface.base'];
		theme?.set('dark');
		flushSync();
		expect(charts).toHaveLength(2);
		expect(charts[0]?.destroy).toHaveBeenCalledTimes(1);
		expect(config.mock.calls.at(-1)?.[0]['color.surface.base']).not.toBe(light);
		expect(screen.getByRole('img', { name: 'Files by size' })).toBeTruthy();
		expect(screen.getByText('a: 1')).toBeTruthy();
		unmount();
		expect(charts[1]?.destroy).toHaveBeenCalledTimes(1);
		theme?.set('system');
	});
});
