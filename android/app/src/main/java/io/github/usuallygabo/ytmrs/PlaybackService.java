package io.github.usuallygabo.ytmrs;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.graphics.drawable.Icon;
import android.os.IBinder;
import android.util.Log;

/**
 * Keeps the process in the foreground while music plays, so Android doesn't stop it
 * with the screen off, and shows the media notification. While paused the notification
 * stays (and can be swiped away) but the service is no longer in the foreground.
 */
public class PlaybackService extends Service {
    private static final String TAG = "ytm-rs";
    private static final String CHANNEL = "playback";
    private static final int ID = 1;
    private static final String ACTION_PREVIOUS = "previous", ACTION_PLAY = "play", ACTION_PAUSE = "pause",
            ACTION_NEXT = "next", ACTION_DISMISSED = "dismissed";

    private static PlaybackService running;

    /** Bring the service and notification in line with the player. */
    static void sync(Context c) {
        Player p = Player.get(c);
        if (running != null) {
            running.refresh(false);
        } else if (p.playing && p.hasTrack) {
            try {
                c.startForegroundService(new Intent(c, PlaybackService.class));
            } catch (Exception e) {
                // Not allowed from the background right now (Android 12+ rules).
                Log.w(TAG, "playback service: " + e);
            }
        }
    }

    @Override
    public void onCreate() {
        super.onCreate();
        running = this;
        NotificationChannel ch = new NotificationChannel(CHANNEL, "Playback", NotificationManager.IMPORTANCE_LOW);
        ch.setShowBadge(false);
        getSystemService(NotificationManager.class).createNotificationChannel(ch);
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        String action = intent != null ? intent.getAction() : null;
        if (action != null) {
            switch (action) {
                // Play and Pause, not a toggle: some systems (HyperOS) send a button press
                // through both the session and this action, and two toggles undo each other.
                case ACTION_PREVIOUS: Player.command("notification previous", Bridge.CMD_PREVIOUS, 0); break;
                case ACTION_PLAY: Player.command("notification play", Bridge.CMD_PLAY, 0); break;
                case ACTION_PAUSE: Player.command("notification pause", Bridge.CMD_PAUSE, 0); break;
                case ACTION_NEXT: Player.command("notification next", Bridge.CMD_NEXT, 0); break;
                case ACTION_DISMISSED:
                    if (!Player.get(this).playing) stopSelf();
                    return START_NOT_STICKY;
            }
        }
        // Started with startForegroundService: it must go to the foreground even if the
        // music paused meanwhile, or Android stops the app.
        refresh(action == null);
        return START_NOT_STICKY;
    }

    private void refresh(boolean promote) {
        Player p = Player.get(this);
        Notification n = build(p);
        if (p.playing || promote) {
            try {
                startForeground(ID, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK);
            } catch (Exception e) {
                Log.w(TAG, "startForeground: " + e);
            }
        }
        if (!p.playing) {
            stopForeground(STOP_FOREGROUND_DETACH);
            if (p.hasTrack) getSystemService(NotificationManager.class).notify(ID, n);
        }
    }

    private PendingIntent action(String name, int code) {
        Intent i = new Intent(this, PlaybackService.class).setAction(name);
        return PendingIntent.getService(this, code, i, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }

    private Notification build(Player p) {
        Notification.Action prev = new Notification.Action.Builder(
                Icon.createWithResource(this, android.R.drawable.ic_media_previous), "Previous", action(ACTION_PREVIOUS, 1)).build();
        Notification.Action toggle = new Notification.Action.Builder(
                Icon.createWithResource(this, p.playing ? android.R.drawable.ic_media_pause : android.R.drawable.ic_media_play),
                p.playing ? "Pause" : "Play", p.playing ? action(ACTION_PAUSE, 2) : action(ACTION_PLAY, 5)).build();
        Notification.Action next = new Notification.Action.Builder(
                Icon.createWithResource(this, android.R.drawable.ic_media_next), "Next", action(ACTION_NEXT, 3)).build();
        Notification.Builder b = new Notification.Builder(this, CHANNEL)
                .setSmallIcon(R.drawable.ic_notification)
                .setContentTitle(p.title)
                .setContentText(p.artist)
                .setContentIntent(p.session.getController().getSessionActivity())
                .setDeleteIntent(action(ACTION_DISMISSED, 4))
                .setVisibility(Notification.VISIBILITY_PUBLIC)
                .setOngoing(p.playing)
                .setShowWhen(false)
                .addAction(prev)
                .addAction(toggle)
                .addAction(next)
                .setStyle(new Notification.MediaStyle()
                        .setMediaSession(p.session.getSessionToken())
                        .setShowActionsInCompactView(0, 1, 2));
        if (p.art != null) b.setLargeIcon(p.art);
        return b.build();
    }

    /** Swiped away from Recents. With this service running some systems (HyperOS) keep
     *  the activity, so this ends the app here: the music stops with it. */
    @Override
    public void onTaskRemoved(Intent rootIntent) {
        Log.i(TAG, "task removed: closing");
        Player.get(this).release();
        stopForeground(STOP_FOREGROUND_REMOVE);
        getSystemService(NotificationManager.class).cancelAll();
        stopSelf();
        android.os.Process.killProcess(android.os.Process.myPid());
    }

    @Override
    public void onDestroy() {
        running = null;
        super.onDestroy();
    }

    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
}
