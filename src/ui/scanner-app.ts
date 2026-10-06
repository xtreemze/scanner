import { LitElement, css, html } from 'lit';
import { customElement, state } from 'lit/decorators.js';
import './scanner-preview';
import './onboarding-flow';
import { platformCapabilities, type PlatformCapabilities } from '../platform/native-bridge';

@customElement('scanner-app')
export class ScannerApp extends LitElement {
  @state() private platform?: PlatformCapabilities;

  static styles = css`
    :host { display: block; }
    header {
      position: sticky;
      top: 0;
      z-index: 10;
      backdrop-filter: blur(18px);
      background: color-mix(in srgb, var(--bg) 76%, transparent);
      border-bottom: 1px solid var(--line);
    }
    .bar {
      max-width: 78rem;
      margin: auto;
      padding: .85rem 1rem;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 1rem;
    }
    .brand {
      font: 700 .92rem/1 var(--mono);
      letter-spacing: .08em;
      text-transform: uppercase;
    }
    nav { display: flex; gap: .9rem; }
    nav a { color: var(--muted); text-decoration: none; font-size: .9rem; }
    main { max-width: 78rem; margin: auto; padding: 0 1rem 5rem; }
    .hero {
      min-height: 78svh;
      display: grid;
      grid-template-columns: 1.05fr .95fr;
      align-items: center;
      gap: 3rem;
      padding: 4rem 0;
    }
    .eyebrow {
      font: 650 .8rem/1.2 var(--mono);
      color: var(--accent);
      letter-spacing: .1em;
      text-transform: uppercase;
    }
    h1 {
      max-width: 14ch;
      margin: .8rem 0 1.1rem;
      font-size: clamp(3rem, 8vw, 6.7rem);
      line-height: .88;
      letter-spacing: -.065em;
    }
    .lede {
      color: var(--muted);
      font-size: clamp(1.05rem, 2vw, 1.35rem);
      max-width: 42rem;
    }
    .actions {
      margin-top: 1.5rem;
      display: flex;
      flex-wrap: wrap;
      gap: .7rem;
    }
    .button {
      display: inline-flex;
      align-items: center;
      border-radius: 999px;
      padding: .78rem 1rem;
      text-decoration: none;
      border: 1px solid var(--line);
      color: var(--text);
    }
    .button.primary { background: var(--text); color: var(--bg); }
    section { padding: 5rem 0; border-top: 1px solid var(--line); }
    h2 {
      max-width: 18ch;
      font-size: clamp(2.2rem, 6vw, 4.6rem);
      line-height: .95;
      letter-spacing: -.045em;
    }
    .grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 1rem; }
    article {
      padding: 1.2rem;
      border: 1px solid var(--line);
      border-radius: 1.2rem;
      background: var(--panel);
    }
    article h3 { margin: .25rem 0 .6rem; }
    article p { color: var(--muted); }
    .metric { font: 600 2rem/1 var(--mono); color: var(--accent); }
    .platform { margin-top: 1.25rem; color: var(--muted); font: 500 .85rem/1.5 var(--mono); }
    @media (max-width: 800px) {
      nav { display: none; }
      .hero {
        grid-template-columns: 1fr;
        gap: 1.5rem;
        min-height: auto;
        padding: 4rem 0 2rem;
      }
      .grid { grid-template-columns: 1fr; }
    }
  `;

  connectedCallback(): void {
    super.connectedCallback();
    void platformCapabilities().then((platform) => {
      this.platform = platform;
    });
  }

  render() {
    const platformLabel = this.platform
      ? [this.platform.runtime, ...this.platform.capabilities].join(' · ')
      : 'detecting platform capabilities…';

    return html`
      <header>
        <div class="bar">
          <div class="brand">Scanner</div>
          <nav aria-label="Primary">
            <a href="#system">System</a>
            <a href="#confidence">Confidence</a>
            <a href="#onboarding">Onboarding</a>
          </nav>
        </div>
      </header>

      <main>
        <div class="hero">
          <div>
            <div class="eyebrow">Spatial reconstruction as an active measurement loop</div>
            <h1>Build the model while you scan.</h1>
            <p class="lede">
              Scanner fuses cameras, motion, depth, LiDAR, ranging, structural surfaces,
              and guided user actions into a model you can inspect and improve in real time.
            </p>
            <div class="actions">
              <a class="button primary" href="#onboarding">Start onboarding</a>
              <a class="button" href="https://github.com/xtreemze/scanner">View source</a>
            </div>
            <div class="platform">${platformLabel}</div>
          </div>
          <scanner-preview></scanner-preview>
        </div>

        <section id="system">
          <div class="eyebrow">One session world</div>
          <h2>Every observation becomes a constraint, not a competing truth.</h2>
          <div class="grid">
            <article>
              <div class="metric">01</div>
              <h3>Structure</h3>
              <p>Walls, floors, ceilings, corners, and stable surfaces become explicit structural constraints with evidence and lock state.</p>
            </article>
            <article>
              <div class="metric">02</div>
              <h3>Peers</h3>
              <p>Phones can scan, anchor, observe, illuminate, and contribute ranging or depth without owning the canonical world frame.</p>
            </article>
            <article>
              <div class="metric">03</div>
              <h3>Guidance</h3>
              <p>The planner chooses the next useful movement, viewpoint, anchor placement, or illumination action by expected information gain.</p>
            </article>
          </div>
        </section>

        <section id="confidence">
          <div class="eyebrow">Confidence is visible</div>
          <h2>A polished texture must never hide uncertain geometry.</h2>
          <div class="grid">
            <article><h3>Geometry</h3><p>Depth consistency, viewpoint diversity, observations, residual error, and locked constraints.</p></article>
            <article><h3>Pose</h3><p>Visual-inertial tracking, ranging geometry, shared landmarks, relocalization, and clock uncertainty.</p></article>
            <article><h3>Appearance</h3><p>Texture resolution, exposure quality, HDR coverage, controlled flash observations, and material confidence.</p></article>
          </div>
        </section>

        <section id="onboarding">
          <div class="eyebrow">Guided setup</div>
          <h2>Scanner can ask the user to improve the measurement.</h2>
          <onboarding-flow></onboarding-flow>
        </section>
      </main>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    'scanner-app': ScannerApp;
  }
}
