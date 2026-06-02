import argparse

import torch
from torch.utils.data import DataLoader

from dataset import SpinSet
from model import SpinNet


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--cache", required=True)
    p.add_argument("--ckpt", default="ml/runs/spinnet.pt")
    args = p.parse_args()

    device = "mps" if torch.backends.mps.is_available() else "cpu"
    ds = SpinSet(args.cache)
    loader = DataLoader(ds, batch_size=128)
    model = SpinNet().to(device)
    model.load_state_dict(torch.load(args.ckpt, map_location=device))
    model.eval()

    rerr, aerr, rpms = [], [], []
    with torch.no_grad():
        for x, n, rpm, axis, fps in loader:
            x, n, rpm, axis, fps = (t.to(device) for t in (x, n, rpm, axis, fps))
            prpm, paxis = model(x, n, fps)
            rerr.append((((prpm - rpm).abs() / rpm) * 100.0).cpu())
            aerr.append((paxis - axis).abs().cpu())
            rpms.append(rpm.cpu())
    rerr = torch.cat(rerr)
    aerr = torch.cat(aerr)
    rpms = torch.cat(rpms)

    print("n=%d  rpm_err mean %.2f%% median %.2f%%  axis_err mean %.2f deg median %.2f deg" % (
        len(rerr), rerr.mean(), rerr.median(), aerr.mean(), aerr.median()))
    for lo, hi, name in [(0, 3500, "low (driver)"), (3500, 7500, "mid (iron)"), (7500, 12000, "high (wedge)")]:
        m = (rpms >= lo) & (rpms < hi)
        if m.sum() > 0:
            print("  %-14s rpm %5d-%5d  n=%3d  rpm_err median %5.1f%%  axis median %4.2f deg" % (
                name, lo, hi, int(m.sum()), rerr[m].median(), aerr[m].median()))


if __name__ == "__main__":
    main()
