import torch
import torch.nn as nn

RPM_SCALE = 6000.0
AXIS_SCALE = 20.0
NMAX = 6


class SpinNet(nn.Module):
    def __init__(self, nmax=NMAX):
        super().__init__()
        # Early fusion: frames as channels, so conv1 compares them (rotation cue).
        self.enc = nn.Sequential(
            nn.Conv2d(nmax, 32, 5, 2, 2), nn.BatchNorm2d(32), nn.ReLU(),
            nn.Conv2d(32, 64, 3, 2, 1), nn.BatchNorm2d(64), nn.ReLU(),
            nn.Conv2d(64, 128, 3, 2, 1), nn.BatchNorm2d(128), nn.ReLU(),
            nn.Conv2d(128, 256, 3, 2, 1), nn.BatchNorm2d(256), nn.ReLU(),
            nn.Conv2d(256, 256, 3, 2, 1), nn.BatchNorm2d(256), nn.ReLU(),
            nn.AdaptiveAvgPool2d(5), nn.Flatten(),
            nn.Linear(256 * 25, 512), nn.ReLU(),
        )
        self.head = nn.Sequential(
            nn.Linear(512 + 2, 256), nn.ReLU(),
            nn.Linear(256, 2),
        )

    def forward(self, x, n, fps):
        feat = self.enc(x)
        extra = torch.stack([fps / 1000.0, n.float() / NMAX], dim=1)
        y = self.head(torch.cat([feat, extra], dim=1))
        rpm = y[:, 0] * RPM_SCALE
        axis = y[:, 1] * AXIS_SCALE
        return rpm, axis
