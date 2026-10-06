import { registerSW } from 'virtual:pwa-register';
import './styles.css';
import './ui/scanner-app';

registerSW({ immediate: true });
