import type { ReactNode } from 'react';
import { contrastRatio } from '../theme/contrast';
import { colorTokens, contrastPairs, type ColorTokenName } from '../theme/tokens';
import {
  AppGlyph,
  Badge,
  Banner,
  Button,
  Card,
  Check,
  CodeDisplay,
  Decision,
  Empty,
  IconButton,
  Kbd,
  List,
  Manifest,
  MapRow,
  OsBadge,
  PermRow,
  Progress,
  Route,
  Row,
  Search,
  Segmented,
  Tick,
  Toast,
  Toggle,
  Tree,
  TreeRow,
} from '../ui';

const noop = (): void => undefined;

const SWATCHES: readonly ColorTokenName[] = [
  'surface-0',
  'surface-1',
  'surface-2',
  'surface-3',
  'text-primary',
  'text-secondary',
  'text-muted',
  'border-strong',
  'control',
  'accent',
  'accent-fill',
  'accent-soft',
  'warning',
  'warning-soft',
  'danger',
  'danger-soft',
  'success',
  'success-soft',
];

function Section({
  title,
  lede,
  children,
}: {
  readonly title: string;
  readonly lede?: string;
  readonly children?: ReactNode;
}) {
  return (
    <section>
      <h2>{title}</h2>
      {lede !== undefined && <p className="p-lede">{lede}</p>}
      {children}
    </section>
  );
}

function Label({ children }: { readonly children: string }) {
  return <p className="g-kit__label">{children}</p>;
}

function Ratio({ value, min }: { readonly value: number; readonly min: number }) {
  const pass = value >= min;
  return (
    <td className={pass ? 'g-kit__ok' : 'g-kit__bad'}>
      {value.toFixed(2)} {pass ? 'pass' : 'FAIL'}
    </td>
  );
}

function Colours() {
  return (
    <>
      <Section
        title="Colour tokens"
        lede="Each swatch: light on the left, dark (Graphite) on the right. Elevation in dark comes from lighter surfaces."
      >
        <div className="g-kit__swatches">
          {SWATCHES.map((name) => (
            <div key={name} className="g-kit__swatch">
              <div className="g-kit__chips">
                <span style={{ background: colorTokens[name].light }} />
                <span style={{ background: colorTokens[name].dark }} />
              </div>
              <div className="g-kit__swatch-text">
                <b>{name}</b>
                <span>
                  {colorTokens[name].light} · {colorTokens[name].dark}
                </span>
              </div>
            </div>
          ))}
        </div>
      </Section>
      <Section
        title="Measured contrast (WCAG 2.x)"
        lede="Computed from the token table, so the numbers can't drift. Text needs 4.5:1; edges, focus and the toast icon need 3:1."
      >
        <table className="g-kit__table">
          <thead>
            <tr>
              <th>Pair</th>
              <th>Needs</th>
              <th>Light</th>
              <th>Dark · Graphite</th>
            </tr>
          </thead>
          <tbody>
            {contrastPairs.map(({ fg, bg, min }) => (
              <tr key={`${fg}/${bg}`}>
                <td>
                  <code>{fg}</code> on <code>{bg}</code>
                </td>
                <td>{min}:1</td>
                <Ratio
                  value={contrastRatio(colorTokens[fg].light, colorTokens[bg].light)}
                  min={min}
                />
                <Ratio
                  value={contrastRatio(colorTokens[fg].dark, colorTokens[bg].dark)}
                  min={min}
                />
              </tr>
            ))}
          </tbody>
        </table>
      </Section>
    </>
  );
}

function Type() {
  return (
    <Section
      title="Type"
      lede="Atkinson Hyperlegible Next and Mono. Ratio 1.25 on 15 px; a display step for the celebration moments."
    >
      <div className="g-kit__type">
        <span>display 36/750</span>
        <b className="p-title p-title--hero">Everything arrived.</b>
        <span>2xl 28/700</span>
        <b className="p-title">Here&apos;s what will change</b>
        <span>lg 18/700</span>
        <b className="g-kit__lg">Bring your SSH keys?</b>
        <span>md 15/400</span>
        <span className="g-kit__md">Nothing on this Mac changes until you approve.</span>
        <span>sm 13/400</span>
        <span className="g-kit__sm">Checked against the sender&apos;s hashes</span>
        <span>mono 13</span>
        <code>~/Library/Application Support/Code/User</code>
      </div>
    </Section>
  );
}

function Controls() {
  return (
    <Section title="Buttons, chips and badges">
      <Label>Buttons</Label>
      <div className="g-kit__row">
        <Button kind="primary">Approve and move</Button>
        <Button>Leave behind</Button>
        <Button kind="quiet">Customize</Button>
        <Button kind="quiet" icon="undo">
          Undo this move…
        </Button>
        <Button kind="danger">Cancel and undo</Button>
        <Button busy disabled>
          Waiting…
        </Button>
        <Button kind="primary" disabled>
          Approve
        </Button>
        <IconButton icon="close" label="Close" />
        <Button kind="primary" size="lg" icon="send">
          Send <Kbd>S</Kbd>
        </Button>
      </div>
      <Label>Chips</Label>
      <div className="g-kit__row">
        <Badge>Cache</Badge>
        <Badge tone="accent">Plugin</Badge>
        <Badge tone="success" icon="check">
          Arrived
        </Badge>
        <Badge tone="warning">Runs code</Badge>
        <Badge tone="danger" icon="close">
          Failed
        </Badge>
        <Badge tone="success" icon="check">
          Verified
        </Badge>
      </div>
      <Label>OS badges</Label>
      <div className="g-kit__row">
        <OsBadge os="linux" label="Ubuntu 24.04" />
        <OsBadge os="mac_os" label="macOS 15" />
        <OsBadge os="windows" label="Windows 11" />
        <OsBadge os="linux" label="Ubuntu" size="sm" />
        <OsBadge os="mac_os" label="macOS" size="sm" />
        <OsBadge os="windows" label="Windows" size="sm" />
        <Route
          from={{ os: 'linux', name: 'ezio-thinkpad' }}
          to={{ os: 'mac_os', name: 'ezio-mbp' }}
        />
        <Route from={{ os: 'linux', name: 'ezio-thinkpad' }} to={null} />
      </div>
      <Label>Controls</Label>
      <div className="g-kit__row">
        <Check value label="Picked" onChange={noop} />
        <Check value="mixed" label="Some picked" onChange={noop} />
        <Check value={false} label="Not picked" onChange={noop} />
        <Check value warn label="Sensitive, picked" onChange={noop} />
        <Toggle on label="On" onChange={noop} />
        <Toggle on={false} label="Off" onChange={noop} />
        <Segmented
          label="Your SSH keys"
          warn
          value="bring"
          options={[
            { value: 'leave', label: 'Leave behind' },
            { value: 'bring', label: 'Bring them' },
          ]}
          onChange={noop}
        />
        <Search label="Filter apps" placeholder="Filter apps" value="" onChange={noop} />
        <Kbd>[</Kbd>
        <Kbd>Esc</Kbd>
      </div>
    </Section>
  );
}

function Rows() {
  return (
    <Section title="Rows">
      <div className="g-kit__grid">
        <div>
          <Label>App row</Label>
          <List label="App row">
            <Row
              lead={
                <>
                  <Check value label="VS Code" onChange={noop} />
                  <AppGlyph glyph="{ }" />
                </>
              }
              title="VS Code"
              sub="Settings, keybindings, snippets"
              trail="2.1 MB"
            />
          </List>
          <Label>Sensitive row</Label>
          <List label="Sensitive row" warn="Keys and secrets start off, asked one by one">
            <Row
              off
              lead={
                <>
                  <Check value={false} warn label="SSH keys" onChange={noop} />
                  <AppGlyph glyph="ssh" tone="warning" />
                </>
              }
              title="SSH keys"
              sub="Start off, asked one by one"
              trail={<Button size="sm">Include…</Button>}
            />
          </List>
          <Label>Progress row</Label>
          <List label="Progress row">
            <Row
              lead={
                <>
                  <Tick state="done" />
                  <AppGlyph glyph="git" size="sm" />
                </>
              }
              title="Git"
              sub="In place"
            />
            <Row
              lead={
                <>
                  <Tick state="now" />
                  <AppGlyph glyph="$_" size="sm" />
                </>
              }
              title="Zsh"
              sub={<code>~/.zshrc</code>}
              trail={<Progress value={0.48} label="Zsh" size="sm" />}
            />
          </List>
        </div>
        <div>
          <Label>Decision row</Label>
          <List label="Decision row">
            <li>
              <Decision
                icon="running"
                title="VS Code is open"
                why="Quit it so its settings can be placed."
                answered={false}
                action={
                  <Button kind="quiet" size="sm">
                    Skip
                  </Button>
                }
              />
            </li>
          </List>
          <Label>Path mapping</Label>
          <Card pad>
            <MapRow
              from={{ os: 'linux', label: 'Ubuntu', path: '~/.config/Code/User' }}
              to={{ os: 'mac_os', label: 'macOS', path: '~/Library/Application Support/Code/User' }}
            />
          </Card>
          <Label>Permission</Label>
          <Card pad>
            <ul>
              <PermRow
                icon="noNet"
                title="No internet"
                detail="It can't send anything anywhere"
                safe
              />
            </ul>
          </Card>
        </div>
      </div>
    </Section>
  );
}

function Feedback() {
  return (
    <Section title="Banners, toasts, empty states">
      <div className="g-kit__grid">
        <div className="p-stack">
          <Banner
            tone="accent"
            title="Moving across systems"
            body="Carrova puts each file where macOS apps look."
          />
          <Banner
            tone="warning"
            title="ezio-thinkpad stopped answering"
            body="24.1 MB is safe in staging."
          />
          <Banner
            tone="danger"
            title="Not enough space"
            body="Needs 1.2 GB, 640 MB free."
            action={<Button size="sm">Show what&apos;s big</Button>}
          />
          <Banner tone="success" title="Back exactly as it was" body="0 problems." />
          <Toast docked={false} onDismiss={noop}>
            Copied <code>brew install neovim</code>
          </Toast>
        </div>
        <div className="p-stack">
          <Empty
            art="history"
            title="No moves yet"
            body="When you send or receive, it shows up here with a way to undo it."
            action={
              <Button kind="primary" size="sm">
                Receive on this computer
              </Button>
            }
          />
          <Empty
            art="search"
            title="We didn't find any apps we know"
            body="You can still bring folders as plain files."
            action={<Button>Pick folders</Button>}
          />
        </div>
      </div>
    </Section>
  );
}

/** Overlays as static previews: the real Sheet and Dialog trap focus, which a sheet of samples must not. */
function Overlays() {
  return (
    <Section title="Overlays and signature pieces">
      <div className="g-kit__grid">
        <div>
          <Label>Sheet (drill-down)</Label>
          <div className="g-kit__mini">
            <div className="p-scrim" />
            <div className="p-sheet">
              <div className="p-sheet__head">
                <b>Customize VS Code</b>
              </div>
              <div className="p-sheet__body">
                <Tree label="Customize VS Code">
                  <TreeRow
                    depth={1}
                    check={<Check value label="settings.json" onChange={noop} />}
                    name={<code>settings.json</code>}
                  />
                  <TreeRow
                    depth={1}
                    off
                    check={<Check value={false} label="History" onChange={noop} />}
                    name={<code>History</code>}
                    meta={<Badge>Cache</Badge>}
                  />
                </Tree>
              </div>
            </div>
          </div>
        </div>
        <div>
          <Label>Dialog (risk)</Label>
          <div className="g-kit__mini">
            <div className="p-scrim" />
            <div className="p-dialog">
              <b>Copy GNOME Terminal profile anyway?</b>
              <p className="p-note">Nothing on this Mac will read it.</p>
              <div className="p-dialog__foot">
                <Button size="sm">Keep it behind</Button>
                <Button kind="primary" size="sm">
                  Copy anyway
                </Button>
              </div>
            </div>
          </div>
        </div>
        <div>
          <Label>Pairing code</Label>
          <Card pad>
            <CodeDisplay words={['7', 'zebra', 'lemon']} />
            <CodeDisplay words={['7', 'zebra', 'lemon']} size="sm" />
          </Card>
        </div>
        <div>
          <Label>Manifest rail</Label>
          <Card pad>
            <Manifest
              from={{
                name: 'ezio-thinkpad',
                note: 'This computer',
                os: { kind: 'linux', label: 'Ubuntu' },
              }}
              to={{
                name: 'ezio-mbp',
                note: 'Paired · reviewing',
                os: { kind: 'mac_os', label: 'macOS 15' },
              }}
              facts={[
                { label: 'Apps', value: '11 of 15' },
                { label: 'Keys', value: 'Left behind', onChange: noop },
              ]}
              promise="Nothing is written on the other computer until someone there approves the plan. Every move can be undone for 30 days."
            />
          </Card>
        </div>
      </div>
    </Section>
  );
}

/** The component sheet (mockup `kit`): tokens, measured contrast and every primitive, in both themes. */
export function Kit() {
  return (
    <article className="g-kit">
      <section>
        <h1 className="p-title p-title--hero">Carrova components</h1>
        <p className="p-lede">
          Layouts: <b>hub</b> for places, <b>flow</b> for tasks (labelled steps, main column,
          manifest rail, one action bar), <b>sheet</b> for detail, <b>dialog</b> for risk. One
          primary button per screen.
        </p>
      </section>
      <Colours />
      <Type />
      <Controls />
      <Rows />
      <Feedback />
      <Overlays />
      <Section
        title="Motion"
        lede="120 / 220 / 420 ms, exponential ease-out; spring only where something lands (code tiles, lock, seal, toast). Under reduced motion every decorative animation stops; progress numbers still update."
      />
    </article>
  );
}
