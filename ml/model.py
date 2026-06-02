import torch
import torch.nn as nn

RPM_SCALE = 6000.0
AXIS_SCALE = 20.0


class SpinNet(nn.Module):
    def __init__(self):
        super().__init__()
        self.enc = nn.Sequential(
            nn.Conv2d(1, 16, 5, 2, 2), nn.ReLU(),
            nn.Conv2d(16, 32, 3, 2, 1), nn.ReLU(),
            nn.Conv2d(32, 64, 3, 2, 1), nn.ReLU(),
            nn.AdaptiveAvgPool2d(4), nn.Flatten(),
            nn.Linear(64 * 16, 128), nn.ReLU(),
        )
        self.gru = nn.GRU(128, 128, batch_first=True)
        self.head = nn.Sequential(
            nn.Linear(128 + 1, 128), nn.ReLU(),
            nn.Linear(128, 2),
        )

    def forward(self, x, n, fps):
        b, nmax, h, w = x.shape
        f = self.enc(x.reshape(b * nmax, 1, h, w)).reshape(b, nmax, -1)
        out, _ = self.gru(f)
        idx = (n - 1).clamp(min=0)
        last = out[torch.arange(b, device=x.device), idx]
        feat = torch.cat([last, (fps / 1000.0).unsqueeze(1)], dim=1)
        y = self.head(feat)
        rpm = y[:, 0] * RPM_SCALE
        axis = y[:, 1] * AXIS_SCALE
        return rpm, axis
