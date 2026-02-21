/*
 * spice_helpers.c — GTK3 + spice-client-gtk UI for grustyvman-viewer.
 *
 * All GTK3/SPICE work lives here to avoid calling variadic C functions
 * (g_object_set, g_signal_connect, etc.) from stable Rust FFI.
 */

#include <spice-client-gtk.h>
#include <gdk/gdkkeysyms.h>
#include <string.h>

/* ---- Action IDs (must match Rust side) --------------------------------- */

#define GRV_ACTION_POWER_ON      0
#define GRV_ACTION_PAUSE         1
#define GRV_ACTION_RESUME        2
#define GRV_ACTION_SHUTDOWN      3
#define GRV_ACTION_REBOOT        4
#define GRV_ACTION_FORCE_STOP    5
#define GRV_ACTION_FORCE_REBOOT  6

typedef void (*GrvActionFn)(int action, void *user_data);

/* ---- Key-combo table --------------------------------------------------- */

typedef struct {
    const char *label;
    guint       keyvals[4];
    int         nkeys;
} GrvKeyCombo;

static const GrvKeyCombo KEY_COMBOS[] = {
    { "Ctrl+Alt+Del",          { GDK_KEY_Control_L, GDK_KEY_Alt_L, GDK_KEY_Delete    }, 3 },
    { "Ctrl+Alt+Backspace",    { GDK_KEY_Control_L, GDK_KEY_Alt_L, GDK_KEY_BackSpace }, 3 },
    { "Ctrl+Alt+F1 (TTY 1)",   { GDK_KEY_Control_L, GDK_KEY_Alt_L, GDK_KEY_F1       }, 3 },
    { "Ctrl+Alt+F2 (TTY 2)",   { GDK_KEY_Control_L, GDK_KEY_Alt_L, GDK_KEY_F2       }, 3 },
    { "Ctrl+Alt+F3 (TTY 3)",   { GDK_KEY_Control_L, GDK_KEY_Alt_L, GDK_KEY_F3       }, 3 },
    { "Ctrl+Alt+F4 (TTY 4)",   { GDK_KEY_Control_L, GDK_KEY_Alt_L, GDK_KEY_F4       }, 3 },
    { "Ctrl+Alt+F5 (TTY 5)",   { GDK_KEY_Control_L, GDK_KEY_Alt_L, GDK_KEY_F5       }, 3 },
    { "Ctrl+Alt+F6 (TTY 6)",   { GDK_KEY_Control_L, GDK_KEY_Alt_L, GDK_KEY_F6       }, 3 },
    { "Ctrl+Alt+F7 (Desktop)", { GDK_KEY_Control_L, GDK_KEY_Alt_L, GDK_KEY_F7       }, 3 },
    { "Print Screen",          { GDK_KEY_Print                                       }, 1 },
    { "Alt+F4",                { GDK_KEY_Alt_L,     GDK_KEY_F4                       }, 2 },
};
#define N_KEY_COMBOS ((int)(sizeof(KEY_COMBOS) / sizeof(KEY_COMBOS[0])))

/* ---- Viewer struct ----------------------------------------------------- */

typedef struct {
    GtkWidget        *window;
    SpiceDisplay     *display;
    SpiceSession     *session;
    SpiceMainChannel *main_channel; /* current main channel, NULL until connected */
    GtkWidget        *toolbar;
    GtkWidget        *stack;        /* GtkStack: "display" | "powered-off" */
    GtkWidget        *status_title; /* GtkLabel on powered-off page */
    GtkWidget        *status_sub;   /* GtkLabel on powered-off page */
    gboolean          fullscreen;
    GrvActionFn       action_fn;
    void             *action_data;
} GrvViewer;

/* ---- Forward declarations --------------------------------------------- */
static void toggle_fullscreen(GrvViewer *v);
static void show_display(GrvViewer *v);

/* ---- CSS for the powered-off page ------------------------------------- */

static const char *POWERED_OFF_CSS =
    ".grv-dark-bg {"
    "  background-color: #1c1c1c;"
    "}"
    ".grv-status-title {"
    "  color: #eeeeee;"
    "  font-size: 22px;"
    "  font-weight: bold;"
    "}"
    ".grv-status-sub {"
    "  color: #888888;"
    "  font-size: 13px;"
    "}";

/* ---- Powered-off page builder ----------------------------------------- */

static GtkWidget *
build_status_page(GrvViewer *v)
{
    /* Apply CSS */
    GtkCssProvider *css = gtk_css_provider_new();
    gtk_css_provider_load_from_data(css, POWERED_OFF_CSS, -1, NULL);
    gtk_style_context_add_provider_for_screen(
        gdk_screen_get_default(),
        GTK_STYLE_PROVIDER(css),
        GTK_STYLE_PROVIDER_PRIORITY_APPLICATION);
    g_object_unref(css);

    /* Outer box fills the area with the dark background */
    GtkWidget *outer = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
    gtk_style_context_add_class(gtk_widget_get_style_context(outer), "grv-dark-bg");

    /* Centered inner box */
    GtkWidget *inner = gtk_box_new(GTK_ORIENTATION_VERTICAL, 12);
    gtk_widget_set_valign(inner, GTK_ALIGN_CENTER);
    gtk_widget_set_halign(inner, GTK_ALIGN_CENTER);
    gtk_box_pack_start(GTK_BOX(outer), inner, TRUE, TRUE, 0);

    /* Icon */
    GtkWidget *icon = gtk_image_new_from_icon_name(
        "system-shutdown-symbolic", GTK_ICON_SIZE_DIALOG);
    gtk_image_set_pixel_size(GTK_IMAGE(icon), 64);
    /* Tint the icon grey so it reads well on dark background */
    gtk_style_context_add_class(gtk_widget_get_style_context(icon), "grv-status-sub");
    gtk_box_pack_start(GTK_BOX(inner), icon, FALSE, FALSE, 0);

    /* Title label (updated dynamically) */
    GtkWidget *title = gtk_label_new("VM Powered Off");
    gtk_style_context_add_class(gtk_widget_get_style_context(title), "grv-status-title");
    gtk_box_pack_start(GTK_BOX(inner), title, FALSE, FALSE, 0);
    v->status_title = title;

    /* Subtitle label (updated dynamically) */
    GtkWidget *sub = gtk_label_new("The virtual machine has stopped.");
    gtk_style_context_add_class(gtk_widget_get_style_context(sub), "grv-status-sub");
    gtk_box_pack_start(GTK_BOX(inner), sub, FALSE, FALSE, 4);
    v->status_sub = sub;

    return outer;
}

/* ---- Show the powered-off / error status page -------------------------- */

/* Safe to call from any GLib main-loop callback. */
static void
show_powered_off(GrvViewer *v, const char *title_text, const char *sub_text)
{
    gtk_label_set_text(GTK_LABEL(v->status_title), title_text);
    gtk_label_set_text(GTK_LABEL(v->status_sub),   sub_text);
    gtk_stack_set_visible_child_name(GTK_STACK(v->stack), "powered-off");

    /* Keep controls enabled so "Power On" can restart the VM from here. */

    /* Append state to window title for taskbar visibility */
    const gchar *cur = gtk_window_get_title(GTK_WINDOW(v->window));
    if (cur && !g_str_has_suffix(cur, " [Powered Off]")) {
        gchar *new_title = g_strdup_printf("%s [Powered Off]", cur);
        gtk_window_set_title(GTK_WINDOW(v->window), new_title);
        g_free(new_title);
    }
}

static void
show_display(GrvViewer *v)
{
    gtk_stack_set_visible_child_name(GTK_STACK(v->stack), "display");

    /* Remove the powered-off suffix if present. */
    const gchar *cur = gtk_window_get_title(GTK_WINDOW(v->window));
    if (cur) {
        const char *suffix = " [Powered Off]";
        gsize cur_len = strlen(cur);
        gsize suffix_len = strlen(suffix);
        if (cur_len > suffix_len && g_str_has_suffix(cur, suffix)) {
            gchar *trimmed = g_strndup(cur, cur_len - suffix_len);
            gtk_window_set_title(GTK_WINDOW(v->window), trimmed);
            g_free(trimmed);
        }
    }
}

/* ---- SPICE channel disconnect detection -------------------------------- */

static void
on_channel_event(SpiceChannel *channel, SpiceChannelEvent event, gpointer data)
{
    /* We only care about the main channel — it's the control plane.
     * When it closes the SPICE server is gone (VM powered off / reset). */
    if (!SPICE_IS_MAIN_CHANNEL(channel))
        return;

    GrvViewer *v = (GrvViewer *)data;

    if (event == SPICE_CHANNEL_OPENED) {
        show_display(v);
    } else if (event == SPICE_CHANNEL_CLOSED) {
        show_powered_off(v,
            "VM Powered Off",
            "The virtual machine has stopped.");
    } else if (event >= SPICE_CHANNEL_ERROR_CONNECT) {
        show_powered_off(v,
            "Connection Lost",
            "The SPICE connection was interrupted.");
    }
}

/* Context passed through g_idle_add so the resize push can be deferred to
 * the next event-loop iteration (by which time GTK will have finished all
 * pending size-allocate passes). */
typedef struct {
    GrvViewer        *viewer;
    SpiceMainChannel *channel;
} AgentResizeCtx;

static gboolean
idle_push_display_size(gpointer user_data)
{
    AgentResizeCtx *ctx = (AgentResizeCtx *)user_data;
    GrvViewer        *v = ctx->viewer;
    SpiceMainChannel *ch = ctx->channel;
    g_slice_free(AgentResizeCtx, ctx);

    if (!v->display)
        return G_SOURCE_REMOVE;

    gboolean agent_connected = FALSE;
    g_object_get(G_OBJECT(ch), "agent-connected", &agent_connected, NULL);
    if (!agent_connected)
        return G_SOURCE_REMOVE;

    GtkAllocation alloc;
    gtk_widget_get_allocation(GTK_WIDGET(v->display), &alloc);
    if (alloc.width <= 1 || alloc.height <= 1)
        return G_SOURCE_REMOVE;

    spice_main_channel_update_display_enabled(ch, 0, TRUE, FALSE);
    spice_main_channel_update_display(ch, 0, 0, 0,
                                      alloc.width, alloc.height, TRUE);
    return G_SOURCE_REMOVE;
}

/* Called whenever the spice-vdagent connection state changes.
 *
 * When the agent first becomes available we push the current display
 * dimensions so the guest resizes immediately — without this the guest
 * keeps its original resolution until the user manually resizes the window.
 * We defer via g_idle_add so that any in-flight GTK size-allocate passes
 * complete before we sample the widget dimensions. */
static void
on_main_agent_update(SpiceMainChannel *channel, gpointer data)
{
    GrvViewer *v = (GrvViewer *)data;
    if (!v->display)
        return;

    gboolean agent_connected = FALSE;
    g_object_get(G_OBJECT(channel), "agent-connected", &agent_connected, NULL);
    if (!agent_connected)
        return;

    AgentResizeCtx *ctx = g_slice_new(AgentResizeCtx);
    ctx->viewer  = v;
    ctx->channel = channel;
    g_idle_add(idle_push_display_size, ctx);
}

/* SpiceSession emits "channel-new" for every channel it creates.
 * We hook "channel-event" on the main channel so we know when it goes away,
 * and "main-agent-update" so we can push the initial display size as soon
 * as spice-vdagent connects on the guest. */
static void
on_channel_new(SpiceSession *session, SpiceChannel *channel, gpointer data)
{
    (void)session;
    if (SPICE_IS_MAIN_CHANNEL(channel)) {
        GrvViewer *v = (GrvViewer *)data;
        v->main_channel = SPICE_MAIN_CHANNEL(channel);
        g_signal_connect(channel, "channel-event",
                         G_CALLBACK(on_channel_event), data);
        g_signal_connect(channel, "main-agent-update",
                         G_CALLBACK(on_main_agent_update), data);
    }
}

/* ---- Window callbacks -------------------------------------------------- */

static void
on_delete_event(GtkWidget *widget, GdkEvent *event, gpointer data)
{
    (void)widget; (void)event; (void)data;
    gtk_main_quit();
}

static gboolean
on_key_press(GtkWidget *widget, GdkEventKey *event, gpointer data)
{
    (void)widget;
    GrvViewer *v = (GrvViewer *)data;
    if (event->keyval == GDK_KEY_F11) {
        toggle_fullscreen(v);
        return TRUE;
    }
    return FALSE;
}

/* ---- Fullscreen toggle ------------------------------------------------- */

static void
toggle_fullscreen(GrvViewer *v)
{
    if (v->fullscreen) {
        gtk_window_unfullscreen(GTK_WINDOW(v->window));
        gtk_widget_show(v->toolbar);
        v->fullscreen = FALSE;
    } else {
        gtk_widget_hide(v->toolbar);
        gtk_window_fullscreen(GTK_WINDOW(v->window));
        v->fullscreen = TRUE;
    }
}

/* ---- Action callbacks -------------------------------------------------- */

static void
on_action(GtkWidget *widget, gpointer data)
{
    int action = GPOINTER_TO_INT(data);
    GrvViewer *v = g_object_get_data(G_OBJECT(widget), "grv-viewer");
    if (v && v->action_fn) {
        v->action_fn(action, v->action_data);
        if (action == GRV_ACTION_POWER_ON) {
            gtk_label_set_text(GTK_LABEL(v->status_title), "Starting VM");
            gtk_label_set_text(GTK_LABEL(v->status_sub), "Waiting for console to reconnect...");
            gtk_stack_set_visible_child_name(GTK_STACK(v->stack), "powered-off");
        }
    }
}

static GtkWidget *
make_action_btn(GrvViewer *v, const char *label, int action)
{
    GtkWidget *btn = gtk_button_new_with_label(label);
    g_object_set_data(G_OBJECT(btn), "grv-viewer", v);
    g_signal_connect(btn, "clicked", G_CALLBACK(on_action), GINT_TO_POINTER(action));
    return btn;
}

/* ---- Popup-menu button helper ------------------------------------------ */

static void
on_popup_clicked(GtkButton *btn, gpointer data)
{
    gtk_menu_popup_at_widget(GTK_MENU(data), GTK_WIDGET(btn),
                             GDK_GRAVITY_SOUTH_WEST, GDK_GRAVITY_NORTH_WEST,
                             NULL);
}

static GtkWidget *
make_popup_btn(const char *label, GtkWidget *menu)
{
    gchar *text = g_strdup_printf("%s \342\226\276", label); /* ▾ U+25BE */
    GtkWidget *btn = gtk_button_new_with_label(text);
    g_free(text);
    g_signal_connect(btn, "clicked", G_CALLBACK(on_popup_clicked), menu);
    return btn;
}

/* ---- Send-key callback ------------------------------------------------- */

static void
on_send_key(GtkMenuItem *item, gpointer data)
{
    (void)data;
    GrvViewer *v   = g_object_get_data(G_OBJECT(item), "grv-viewer");
    int        idx = GPOINTER_TO_INT(g_object_get_data(G_OBJECT(item), "grv-combo-idx"));
    if (!v || idx < 0 || idx >= N_KEY_COMBOS)
        return;
    const GrvKeyCombo *combo = &KEY_COMBOS[idx];
    spice_display_send_keys(v->display,
                            combo->keyvals, combo->nkeys,
                            SPICE_DISPLAY_KEY_EVENT_CLICK);
}

/* ---- View callbacks ---------------------------------------------------- */

static void
on_scale_toggled(GtkCheckMenuItem *item, gpointer data)
{
    (void)data;
    GrvViewer *v = g_object_get_data(G_OBJECT(item), "grv-viewer");
    if (v)
        g_object_set(G_OBJECT(v->display),
                     "scaling", gtk_check_menu_item_get_active(item), NULL);
}

static void
on_resize_guest_toggled(GtkCheckMenuItem *item, gpointer data)
{
    (void)data;
    GrvViewer *v = g_object_get_data(G_OBJECT(item), "grv-viewer");
    if (!v)
        return;

    gboolean active = gtk_check_menu_item_get_active(item);
    g_object_set(G_OBJECT(v->display), "resize-guest", active, NULL);

    /* When enabling, immediately push the current window size to the guest so
     * it resizes right away without waiting for the next window-resize event. */
    if (active && v->main_channel) {
        AgentResizeCtx *ctx = g_slice_new(AgentResizeCtx);
        ctx->viewer  = v;
        ctx->channel = v->main_channel;
        g_idle_add(idle_push_display_size, ctx);
    }
}

static void
on_fullscreen_item(GtkMenuItem *item, gpointer data)
{
    (void)data;
    GrvViewer *v = g_object_get_data(G_OBJECT(item), "grv-viewer");
    if (v)
        toggle_fullscreen(v);
}

/* ---- Toolbar builder --------------------------------------------------- */

static GtkWidget *
build_toolbar(GrvViewer *v)
{
    GtkWidget *bar = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 4);
    gtk_widget_set_margin_start(bar, 6);
    gtk_widget_set_margin_end(bar, 6);
    gtk_widget_set_margin_top(bar, 3);
    gtk_widget_set_margin_bottom(bar, 3);
    gtk_style_context_add_class(gtk_widget_get_style_context(bar), "toolbar");

    /* ── VM control buttons ────────────────────────────────────────────── */
    gtk_box_pack_start(GTK_BOX(bar),
        make_action_btn(v, "Power On", GRV_ACTION_POWER_ON), FALSE, FALSE, 0);
    gtk_box_pack_start(GTK_BOX(bar),
        make_action_btn(v, "Pause",    GRV_ACTION_PAUSE),    FALSE, FALSE, 0);
    gtk_box_pack_start(GTK_BOX(bar),
        make_action_btn(v, "Resume",   GRV_ACTION_RESUME),   FALSE, FALSE, 0);
    gtk_box_pack_start(GTK_BOX(bar),
        make_action_btn(v, "Shutdown", GRV_ACTION_SHUTDOWN), FALSE, FALSE, 0);

    /* ── More dropdown ─────────────────────────────────────────────────── */
    {
        GtkWidget *menu = gtk_menu_new();
        const struct { const char *label; int action; } items[] = {
            { "Reboot",         GRV_ACTION_REBOOT       },
            { "Force Shutdown", GRV_ACTION_FORCE_STOP   },
            { "Force Reboot",   GRV_ACTION_FORCE_REBOOT },
        };
        for (int i = 0; i < 3; i++) {
            GtkWidget *it = gtk_menu_item_new_with_label(items[i].label);
            g_object_set_data(G_OBJECT(it), "grv-viewer", v);
            g_signal_connect(it, "activate",
                             G_CALLBACK(on_action), GINT_TO_POINTER(items[i].action));
            gtk_menu_shell_append(GTK_MENU_SHELL(menu), it);
        }
        gtk_widget_show_all(menu);
        gtk_box_pack_start(GTK_BOX(bar),
            make_popup_btn("More", menu), FALSE, FALSE, 0);
    }

    /* ── Separator ─────────────────────────────────────────────────────── */
    gtk_box_pack_start(GTK_BOX(bar),
        gtk_separator_new(GTK_ORIENTATION_VERTICAL), FALSE, FALSE, 4);

    /* ── Send Key dropdown ─────────────────────────────────────────────── */
    {
        GtkWidget *menu = gtk_menu_new();
        for (int i = 0; i < N_KEY_COMBOS; i++) {
            GtkWidget *it = gtk_menu_item_new_with_label(KEY_COMBOS[i].label);
            g_object_set_data(G_OBJECT(it), "grv-viewer", v);
            g_object_set_data(G_OBJECT(it), "grv-combo-idx", GINT_TO_POINTER(i));
            g_signal_connect(it, "activate", G_CALLBACK(on_send_key), NULL);
            gtk_menu_shell_append(GTK_MENU_SHELL(menu), it);
        }
        gtk_widget_show_all(menu);
        gtk_box_pack_start(GTK_BOX(bar),
            make_popup_btn("Send Key", menu), FALSE, FALSE, 0);
    }

    /* ── View dropdown ─────────────────────────────────────────────────── */
    {
        GtkWidget *menu = gtk_menu_new();

        GtkWidget *scale_it = gtk_check_menu_item_new_with_label("Scale Display");
        gtk_check_menu_item_set_active(GTK_CHECK_MENU_ITEM(scale_it), FALSE);
        g_object_set_data(G_OBJECT(scale_it), "grv-viewer", v);
        g_signal_connect(scale_it, "toggled", G_CALLBACK(on_scale_toggled), NULL);
        gtk_menu_shell_append(GTK_MENU_SHELL(menu), scale_it);

        GtkWidget *resize_it = gtk_check_menu_item_new_with_label("Auto Resize VM");
        gtk_check_menu_item_set_active(GTK_CHECK_MENU_ITEM(resize_it), TRUE);
        g_object_set_data(G_OBJECT(resize_it), "grv-viewer", v);
        g_signal_connect(resize_it, "toggled", G_CALLBACK(on_resize_guest_toggled), NULL);
        gtk_menu_shell_append(GTK_MENU_SHELL(menu), resize_it);

        gtk_menu_shell_append(GTK_MENU_SHELL(menu),
                              gtk_separator_menu_item_new());

        GtkWidget *fs_it = gtk_menu_item_new_with_label("Fullscreen  (F11)");
        g_object_set_data(G_OBJECT(fs_it), "grv-viewer", v);
        g_signal_connect(fs_it, "activate", G_CALLBACK(on_fullscreen_item), NULL);
        gtk_menu_shell_append(GTK_MENU_SHELL(menu), fs_it);

        gtk_widget_show_all(menu);
        gtk_box_pack_start(GTK_BOX(bar),
            make_popup_btn("View", menu), FALSE, FALSE, 0);
    }

    return bar;
}

/* ======================================================================= */
/* Public API (called from Rust)                                            */
/* ======================================================================= */

SpiceSession *
grv_session_create(const char *host, const char *port, const char *password)
{
    SpiceSession *session = spice_session_new();
    g_object_set(G_OBJECT(session), "host", host, "port", port, NULL);
    if (password && *password)
        g_object_set(G_OBJECT(session), "password", password, NULL);
    return session;
}

void
grv_session_connect(SpiceSession *session)
{
    spice_audio_get(session, NULL);
    spice_session_connect(session);
}

void
grv_viewer_reconnect(GrvViewer *v,
                     const char *host,
                     const char *port,
                     const char *password)
{
    if (!v || !v->session || !host || !port)
        return;

    gboolean scaling = FALSE;
    gboolean resize_guest = TRUE;
    SpiceSession *old_session = v->session;
    if (v->display) {
        g_object_get(G_OBJECT(v->display),
                     "scaling", &scaling,
                     "resize-guest", &resize_guest,
                     NULL);
        gtk_container_remove(GTK_CONTAINER(v->stack), GTK_WIDGET(v->display));
        gtk_widget_destroy(GTK_WIDGET(v->display));
        v->display = NULL;
    }

    /* Build a fresh SPICE session/display pair to avoid stale channel state
     * after a full VM power cycle. */
    v->main_channel = NULL; /* old channel is gone; on_channel_new will repopulate */
    spice_session_disconnect(old_session);

    SpiceSession *session = spice_session_new();
    g_object_set(G_OBJECT(session),
                 "host", host,
                 "port", port,
                 NULL);
    if (password && *password) {
        g_object_set(G_OBJECT(session), "password", password, NULL);
    }

    g_signal_connect(session, "channel-new",
                     G_CALLBACK(on_channel_new), v);

    SpiceDisplay *display = spice_display_new(session, 0);
    g_object_set(G_OBJECT(display),
                 "scaling", scaling,
                 "resize-guest", resize_guest,
                 NULL);
    gtk_stack_add_named(GTK_STACK(v->stack), GTK_WIDGET(display), "display");
    /* The window is already shown; newly added children must be shown explicitly
     * so they receive a size allocation before on_main_agent_update fires. */
    gtk_widget_show(GTK_WIDGET(display));
    v->session = session;
    v->display = display;

    g_printerr("grustyvman-viewer: reconnect session to %s:%s\n", host, port);
    spice_audio_get(session, NULL);
    spice_session_connect(session);

    g_object_unref(old_session);
}

GrvViewer *
grv_viewer_build(const char *title,
                 SpiceSession *session,
                 GrvActionFn   action_fn,
                 void         *action_data)
{
    GrvViewer *v   = g_new0(GrvViewer, 1);
    v->session     = session;
    v->action_fn   = action_fn;
    v->action_data = action_data;

    /* Display */
    SpiceDisplay *display = spice_display_new(session, 0);
    g_object_set(G_OBJECT(display),
                 "scaling",      FALSE,
                 "resize-guest", TRUE,
                 NULL);
    v->display = display;

    /* Hook session channel events so we know when the VM dies */
    g_signal_connect(session, "channel-new",
                     G_CALLBACK(on_channel_new), v);

    /* Top-level window */
    GtkWidget *window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
    gtk_window_set_title(GTK_WINDOW(window), title);
    gtk_window_set_default_size(GTK_WINDOW(window), 1024, 768);
    g_signal_connect(window, "delete-event",    G_CALLBACK(on_delete_event), NULL);
    g_signal_connect(window, "key-press-event", G_CALLBACK(on_key_press),    v);
    v->window = window;

    /* Stack: "display" page (SpiceDisplay) and "powered-off" page */
    GtkWidget *stack = gtk_stack_new();
    gtk_stack_set_transition_type(GTK_STACK(stack),
                                  GTK_STACK_TRANSITION_TYPE_CROSSFADE);
    gtk_stack_set_transition_duration(GTK_STACK(stack), 300);
    gtk_stack_add_named(GTK_STACK(stack), GTK_WIDGET(display), "display");
    gtk_stack_add_named(GTK_STACK(stack), build_status_page(v), "powered-off");
    gtk_stack_set_visible_child_name(GTK_STACK(stack), "display");
    v->stack = stack;

    /* Layout: toolbar + separator + stack */
    GtkWidget *vbox    = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
    GtkWidget *toolbar = build_toolbar(v);
    v->toolbar = toolbar;
    gtk_box_pack_start(GTK_BOX(vbox), toolbar,                      FALSE, FALSE, 0);
    gtk_box_pack_start(GTK_BOX(vbox), gtk_separator_new(GTK_ORIENTATION_HORIZONTAL),
                                                                     FALSE, FALSE, 0);
    gtk_box_pack_start(GTK_BOX(vbox), stack,                        TRUE,  TRUE,  0);
    gtk_container_add(GTK_CONTAINER(window), vbox);

    return v;
}

void
grv_viewer_show(GrvViewer *v)
{
    gtk_widget_show_all(v->window);
}

/* Called from the Rust libvirt-polling thread via g_idle_add to switch the
 * viewer to the powered-off page.  Must be invoked on the GTK main thread. */
void
grv_viewer_set_powered_off(GrvViewer *v)
{
    show_powered_off(v, "VM Powered Off",
                     "The virtual machine has stopped.");
}
