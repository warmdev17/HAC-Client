import { invoke, isTauri } from '@tauri-apps/api/core';
import { mount } from './app';
import './styles.css';

mount(document.querySelector<HTMLDivElement>('#app')!, { invoke, desktop: isTauri() });
