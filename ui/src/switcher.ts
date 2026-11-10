// Entry point of the `switcher` window (switcher.html): only the layout
// switcher deck, nothing of the editor or settings, so the window starts fast.
import { mount } from 'svelte';
import '@fontsource-variable/inter/wght.css';
import './app.css';
import Switcher from './components/switcher/Switcher.svelte';
import { demoControls } from './lib/backend';

// Demo mode only: the same hooks as the main window (window.__krovaDemo).
if (demoControls) {
  Object.assign(window, { __krovaDemo: demoControls });
}

const target = document.getElementById('app');
if (!target) throw new Error('#app not found');

export default mount(Switcher, { target });
