import argparse
import torch
from torch.utils.data import DataLoader, random_split

from dataset import SpinSet
from model import SpinNet


def evaluate(model, loader, device):
    model.eval()
    rpm_errs = []
    axis_errs = []
    with torch.no_grad():
        for x, n, rpm, axis, fps in loader:
            x, n, rpm, axis, fps = x.to(device), n.to(device), rpm.to(device), axis.to(device), fps.to(device)
            prpm, paxis = model(x, n, fps)
            rpm_errs.append((((prpm - rpm).abs() / rpm) * 100.0).cpu())
            axis_errs.append((paxis - axis).abs().cpu())
    rpm_errs = torch.cat(rpm_errs)
    axis_errs = torch.cat(axis_errs)
    return rpm_errs.mean().item(), rpm_errs.median().item(), axis_errs.mean().item()


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--cache", required=True)
    p.add_argument("--epochs", type=int, default=60)
    p.add_argument("--batch", type=int, default=64)
    p.add_argument("--lr", type=float, default=1e-3)
    p.add_argument("--out", default="ml/runs/spinnet.pt")
    args = p.parse_args()

    device = "mps" if torch.backends.mps.is_available() else "cpu"
    print("device:", device)

    full = SpinSet(args.cache)
    n_val = max(1, int(0.15 * len(full)))
    n_train = len(full) - n_val
    train_set, val_set = random_split(full, [n_train, n_val], generator=torch.Generator().manual_seed(0))
    train_loader = DataLoader(train_set, batch_size=args.batch, shuffle=True)
    val_loader = DataLoader(val_set, batch_size=args.batch)
    print("train %d / val %d" % (n_train, n_val))

    model = SpinNet().to(device)
    opt = torch.optim.Adam(model.parameters(), lr=args.lr)
    sched = torch.optim.lr_scheduler.CosineAnnealingLR(opt, args.epochs)

    best = 1e9
    for epoch in range(args.epochs):
        model.train()
        for x, n, rpm, axis, fps in train_loader:
            x, n, rpm, axis, fps = x.to(device), n.to(device), rpm.to(device), axis.to(device), fps.to(device)
            x = x + 0.04 * torch.randn_like(x)
            x = x * (0.85 + 0.3 * torch.rand(x.size(0), 1, 1, 1, device=device))
            x = x.clamp(0.0, 1.0)
            prpm, paxis = model(x, n, fps)
            loss = ((prpm - rpm).abs() / rpm).mean() + 0.05 * (paxis - axis).abs().mean()
            opt.zero_grad()
            loss.backward()
            opt.step()
        sched.step()
        rmean, rmed, amean = evaluate(model, val_loader, device)
        if rmean < best:
            best = rmean
            torch.save(model.state_dict(), args.out)
        if epoch % 5 == 0 or epoch == args.epochs - 1:
            tmean, _, _ = evaluate(model, train_loader, device)
            print("epoch %3d  train %6.2f%%  val rpm_err mean %6.2f%% median %6.2f%%  axis_err %5.2f deg" % (epoch, tmean, rmean, rmed, amean))

    print("best val rpm_err mean: %.2f%%  (saved %s)" % (best, args.out))


if __name__ == "__main__":
    main()
