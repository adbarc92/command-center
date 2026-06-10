import { describe, it, expect, afterEach, vi } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/svelte';
import ApprovalOverlay, { type ApprovalRequest } from './ApprovalOverlay.svelte';
import { newUnit } from './fleet';
import type { CreateReq } from './api';

afterEach(cleanup);

function oracleReq(): Extract<ApprovalRequest, { kind: 'oracle' }> {
  const unit = newUnit('u1', 'add sum(a,b) to src/index.js', 'T1');
  unit.phase = 'awaiting_oracle_approval';
  unit.oracleFiles = ['test/sum.test.js', 'test/edge.test.js'];
  return { kind: 'oracle', unit };
}

function launchReq(): Extract<ApprovalRequest, { kind: 'launch' }> {
  const req: CreateReq = { task: 'ship the thing', tier: 't2', mode: 'real', min_review_rounds: 3 };
  return { kind: 'launch', req };
}

describe('ApprovalOverlay.svelte', () => {
  it('renders nothing when there is no request', () => {
    const { queryByTestId } = render(ApprovalOverlay, {
      props: { request: null, onapprove: () => {}, onreject: () => {} },
    });
    expect(queryByTestId('approval-modal')).toBeNull();
    expect(queryByTestId('approval-overlay')).toBeNull();
  });

  it('shows the oracle modal with the frozen test set and APPROVE/REJECT', () => {
    const { getByTestId } = render(ApprovalOverlay, {
      props: { request: oracleReq(), onapprove: () => {}, onreject: () => {} },
    });
    const modal = getByTestId('approval-modal');
    expect(modal.getAttribute('role')).toBe('alertdialog');
    expect(modal.getAttribute('aria-modal')).toBe('true');
    expect(getByTestId('approval-modal').textContent).toContain('ORACLE APPROVAL REQUIRED');

    const files = getByTestId('approval-files');
    expect(files.textContent).toContain('test/sum.test.js');
    expect(files.textContent).toContain('test/edge.test.js');

    expect(getByTestId('approval-approve').textContent).toContain('APPROVE');
    expect(getByTestId('approval-reject').textContent).toContain('REJECT');
  });

  it('fires onapprove / onreject when the oracle buttons are clicked', async () => {
    const onapprove = vi.fn();
    const onreject = vi.fn();
    const { getByTestId } = render(ApprovalOverlay, {
      props: { request: oracleReq(), onapprove, onreject },
    });
    await fireEvent.click(getByTestId('approval-approve'));
    expect(onapprove).toHaveBeenCalledOnce();
    await fireEvent.click(getByTestId('approval-reject'));
    expect(onreject).toHaveBeenCalledOnce();
  });

  it('shows the REAL-launch confirm modal with params and CONFIRM/CANCEL', () => {
    const { getByTestId } = render(ApprovalOverlay, {
      props: { request: launchReq(), onapprove: () => {}, onreject: () => {} },
    });
    const modal = getByTestId('approval-modal');
    expect(modal.textContent).toContain('CONFIRM REAL LAUNCH');
    expect(modal.textContent).toContain('REAL');
    expect(modal.textContent).toContain('T2');
    expect(modal.textContent).toContain('3'); // review rounds
    expect(getByTestId('approval-approve').textContent).toContain('LAUNCH');
    expect(getByTestId('approval-reject').textContent).toContain('CANCEL');
  });

  it('steals focus to the dialog when a request opens', async () => {
    const { getByTestId } = render(ApprovalOverlay, {
      props: { request: oracleReq(), onapprove: () => {}, onreject: () => {} },
    });
    // The $effect + tick run a microtask after mount; give it a turn.
    await Promise.resolve();
    await Promise.resolve();
    expect(document.activeElement).toBe(getByTestId('approval-modal'));
  });

  it('approves on Enter and rejects on Escape (host keeps keyboard control)', async () => {
    const onapprove = vi.fn();
    const onreject = vi.fn();
    const { getByTestId } = render(ApprovalOverlay, {
      props: { request: oracleReq(), onapprove, onreject },
    });
    const overlay = getByTestId('approval-overlay');
    await fireEvent.keyDown(overlay, { key: 'Enter' });
    expect(onapprove).toHaveBeenCalledOnce();
    await fireEvent.keyDown(overlay, { key: 'Escape' });
    expect(onreject).toHaveBeenCalledOnce();
  });
});
