// Entry of the quick panel window (panel.html): the small card the notch opens.
import { mount } from 'svelte';
import '@fontsource-variable/inter/wght.css';
import './app.css';
import Panel from './components/panel/Panel.svelte';
import { demoControls } from './lib/backend';
import { panelDemo } from './lib/panelWindow';

// Demo mode only: hooks for design review and e2e tests, e.g.
//   window.__krovaDemo.panelAnchor({ x, y, w, h, corner: 'bottom_left', monitor_id: 'monitor-1' })
//   window.__krovaPanel.opened  // what the panel asked the main window to show
if (demoControls) Object.assign(window, { __krovaDemo: demoControls, __krovaPanel: panelDemo });

const target = document.getElementById('app');
if (!target) throw new Error('#app not found');

export default mount(Panel, { target });
