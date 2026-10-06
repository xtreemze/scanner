import { LitElement, css, html } from 'lit';
import { customElement } from 'lit/decorators.js';

@customElement('scanner-preview')
export class ScannerPreview extends LitElement {
  static styles = css`
    :host { display: block; min-height: 22rem; }
    canvas {
      width: 100%;
      height: 100%;
      min-height: 22rem;
      display: block;
      border-radius: 1.5rem;
      background: linear-gradient(180deg, #11161b, #090b0d);
    }
  `;

  private frame = 0;
  private raf = 0;

  firstUpdated(): void {
    const canvas = this.renderRoot.querySelector('canvas');
    if (!(canvas instanceof HTMLCanvasElement)) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const draw = () => {
      const rect = canvas.getBoundingClientRect();
      const dpr = Math.min(devicePixelRatio, 2);
      canvas.width = Math.max(1, Math.round(rect.width * dpr));
      canvas.height = Math.max(1, Math.round(rect.height * dpr));
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, rect.width, rect.height);

      const cx = rect.width / 2;
      const cy = rect.height / 2;
      const radius = Math.min(rect.width, rect.height) * 0.27;

      for (let i = 0; i < 520; i += 1) {
        const a = i * 2.399963 + this.frame * 0.002;
        const y = 1 - (i / 519) * 2;
        const r = Math.sqrt(Math.max(0, 1 - y * y));
        const x3 = Math.cos(a) * r;
        const z3 = Math.sin(a) * r;
        const perspective = 1 / (1.8 + z3 * 0.45);
        const x = cx + x3 * radius * perspective * 1.8;
        const y2 = cy + y * radius * perspective * 1.8;
        const confidence = (Math.sin(i * 0.31 + this.frame * 0.01) + 1) / 2;

        ctx.globalAlpha = 0.25 + confidence * 0.7;
        ctx.fillStyle = confidence > 0.58 ? '#9fe8d7' : '#eef2f5';
        ctx.beginPath();
        ctx.arc(x, y2, 0.8 + perspective * 1.8, 0, Math.PI * 2);
        ctx.fill();
      }

      ctx.globalAlpha = 1;
      this.frame += 1;
      this.raf = requestAnimationFrame(draw);
    };

    draw();
  }

  disconnectedCallback(): void {
    cancelAnimationFrame(this.raf);
    super.disconnectedCallback();
  }

  render() {
    return html`<canvas aria-label="Simulated progressive point-cloud preview"></canvas>`;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'scanner-preview': ScannerPreview;
  }
}
