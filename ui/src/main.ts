import { mount } from 'svelte';
import './app.css';
import App from './App.svelte';
import { demoControls } from './lib/backend';

// Demo mode only: hooks for design review and e2e tests, e.g.
//   window.__krovaDemo.navigate('settings')
//   window.__krovaDemo.setPermission('denied')
if (demoControls) {
  Object.assign(window, { __krovaDemo: demoControls });
}

const target = document.getElementById('app');
if (!target) throw new Error('#app not found');

const app = mount(App, { target });

export default app;
