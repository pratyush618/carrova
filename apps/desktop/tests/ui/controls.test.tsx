import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import {
  Banner,
  Button,
  Check,
  Decision,
  Field,
  Input,
  MapRow,
  Progress,
  Segmented,
  Toast,
  Toggle,
} from '@/ui';

describe('Button', () => {
  it('is a button named by its label that cannot be pressed while disabled', async () => {
    const onClick = vi.fn();
    render(
      <Button kind="primary" disabled onClick={onClick}>
        Approve and move
      </Button>,
    );
    const button = screen.getByRole('button', { name: 'Approve and move' });
    await userEvent.click(button);
    expect(button).toBeDisabled();
    expect(onClick).not.toHaveBeenCalled();
  });
});

describe('Button busy', () => {
  it('tells assistive tech it is waiting', () => {
    render(
      <Button busy disabled>
        Waiting…
      </Button>,
    );
    expect(screen.getByRole('button', { name: 'Waiting…' })).toHaveAttribute('aria-busy', 'true');
  });
});

describe('Toggle', () => {
  function Controlled() {
    const [on, setOn] = useState(false);
    return <Toggle on={on} label="Include caches" onChange={setOn} />;
  }

  it('is a switch that flips on click and with Space', async () => {
    const user = userEvent.setup();
    render(<Controlled />);
    const toggle = screen.getByRole('switch', { name: 'Include caches' });
    expect(toggle).not.toBeChecked();
    await user.click(toggle);
    expect(toggle).toBeChecked();
    await user.keyboard(' ');
    expect(toggle).not.toBeChecked();
  });
});

describe('Check', () => {
  it('reports mixed state and picks everything when a mixed box is clicked', async () => {
    const onChange = vi.fn();
    render(<Check value="mixed" label="SSH" onChange={onChange} />);
    const box = screen.getByRole('checkbox', { name: 'SSH' });
    expect(box).toHaveAttribute('aria-checked', 'mixed');
    await userEvent.click(box);
    expect(onChange).toHaveBeenCalledWith(true);
  });
});

describe('Segmented', () => {
  function Controlled({ dir = 'ltr' }: { readonly dir?: 'ltr' | 'rtl' }) {
    const [value, setValue] = useState<'leave' | 'bring' | null>(null);
    return (
      <div dir={dir}>
        <Segmented
          label="Your SSH keys"
          options={[
            { value: 'leave', label: 'Leave behind' },
            { value: 'bring', label: 'Bring them' },
          ]}
          value={value}
          onChange={setValue}
        />
      </div>
    );
  }

  it('starts unanswered with a single tab stop', () => {
    render(<Controlled />);
    const radios = screen.getAllByRole('radio');
    expect(screen.getByRole('radiogroup', { name: 'Your SSH keys' })).toBeInTheDocument();
    expect(radios.every((r) => r.getAttribute('aria-checked') === 'false')).toBe(true);
    expect(radios.map((r) => r.tabIndex)).toEqual([0, -1]);
  });

  it('moves the answer with arrow keys', async () => {
    const user = userEvent.setup();
    render(<Controlled />);
    await user.click(screen.getByRole('radio', { name: 'Leave behind' }));
    await user.keyboard('{ArrowRight}');
    expect(screen.getByRole('radio', { name: 'Bring them' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    expect(screen.getByRole('radio', { name: 'Bring them' })).toHaveFocus();
  });

  it('mirrors left and right in right-to-left layouts', async () => {
    const user = userEvent.setup();
    render(<Controlled dir="rtl" />);
    await user.click(screen.getByRole('radio', { name: 'Leave behind' }));
    await user.keyboard('{ArrowLeft}');
    expect(screen.getByRole('radio', { name: 'Bring them' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
  });
});

describe('Banner', () => {
  it('interrupts for danger and stays polite otherwise', () => {
    render(
      <>
        <Banner tone="danger" title="400 MB more space needed" />
        <Banner tone="accent" title="The other computer can't find this one" />
      </>,
    );
    expect(screen.getByRole('alert')).toHaveTextContent('400 MB more space needed');
    expect(screen.getByRole('status')).toHaveTextContent("can't find this one");
  });
});

describe('Progress', () => {
  it('exposes a clamped percentage to assistive tech', () => {
    render(<Progress value={1.7} label="Overall progress" />);
    expect(screen.getByRole('progressbar', { name: 'Overall progress' })).toHaveAttribute(
      'aria-valuenow',
      '100',
    );
  });
});

describe('Field', () => {
  it('labels its control and announces an error in place of the hint', () => {
    render(
      <Field
        label="Passphrase"
        hint="Four words or more"
        error="That passphrase doesn't open this file."
      >
        {(control) => <Input {...control} type="password" />}
      </Field>,
    );
    const input = screen.getByLabelText('Passphrase');
    expect(input).toHaveAttribute('aria-invalid', 'true');
    expect(input).toHaveAccessibleDescription("That passphrase doesn't open this file.");
    expect(screen.queryByText('Four words or more')).not.toBeInTheDocument();
  });
});

describe('Decision', () => {
  it('turns its tile green once answered, whichever way it is answered', () => {
    const { container, rerender } = render(
      <Decision
        icon="running"
        title="VS Code is open"
        why="Quit it so its settings can be placed."
        answered={false}
        action={<Button size="sm">Skip</Button>}
      />,
    );
    expect(container.querySelector('.p-app--success')).toBeNull();
    rerender(
      <Decision
        icon="key"
        title="Your SSH keys"
        why="Anyone with a copy can log in as you."
        options={[{ value: 'leave', label: 'Leave behind' }]}
        value="leave"
        onChange={() => undefined}
      />,
    );
    expect(container.querySelector('.p-app--success')).not.toBeNull();
    expect(screen.getByRole('radiogroup', { name: 'Your SSH keys' })).toBeInTheDocument();
  });

  it('reads amber while a sensitive question is unanswered', () => {
    const { container } = render(
      <Decision
        icon="key"
        sensitive
        title="Your SSH keys"
        why="Anyone with a copy can log in as you."
        options={[{ value: 'leave', label: 'Leave behind' }]}
        value={null}
        onChange={() => undefined}
      />,
    );
    expect(container.querySelector('.p-app--warning')).not.toBeNull();
  });
});

describe('MapRow', () => {
  it('says when an item lands in the same place', () => {
    render(
      <MapRow
        from={{ os: 'linux', label: 'Ubuntu', path: '~/.gitconfig' }}
        to={{ os: 'mac_os', label: 'macOS', path: '~/.gitconfig' }}
      />,
    );
    expect(screen.getByText('same place')).toBeInTheDocument();
    expect(document.querySelector('mark')).toBeNull();
  });

  it('marks the changed segment on both sides', () => {
    render(
      <MapRow
        from={{ os: 'linux', label: 'Ubuntu', path: '~/.config/Code/User' }}
        to={{ os: 'mac_os', label: 'macOS', path: '~/Library/Application Support/Code/User' }}
      />,
    );
    expect(Array.from(document.querySelectorAll('mark'), (m) => m.textContent)).toEqual([
      '.config',
      'Library/Application Support',
    ]);
  });
});

describe('Toast', () => {
  it('announces politely and can be dismissed', async () => {
    const onDismiss = vi.fn();
    render(<Toast onDismiss={onDismiss}>Copied</Toast>);
    expect(screen.getByRole('status')).toHaveTextContent('Copied');
    await userEvent.click(screen.getByRole('button', { name: 'Dismiss' }));
    expect(onDismiss).toHaveBeenCalledOnce();
  });
});
