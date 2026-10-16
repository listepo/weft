using System;
using System.Linq;
using System.Threading.Tasks;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using SlintBindings.WinUI;
using Windows.System;
using Windows.UI.Core;
using WeftStudio.Model;

namespace WeftStudio.WinUI;

public sealed partial class MainWindow : Window
{
    private readonly Document _document;
    private bool _dialogOpen;

    public MainWindow(string? screen, string? pick)
    {
        InitializeComponent();
        IFilePicker picker = pick is null ? new WinUiFilePicker(AppWindow.Id) : new FixedFilePicker(pick);
        _document = new Document(new Editor(), picker);
        // The event comes from inside the ws_* call; the title is read once that call has returned.
        _document.Changed += () => DispatcherQueue.TryEnqueue(() => Title = _document.Title);
        Closed += (_, _) =>
        {
            Surface.Child = null;
            _document.Dispose();
        };
        // SlintPanel marks every Control chord handled on PreviewKeyDown, which keeps XAML from
        // processing the menu's keyboard accelerators while the panel has focus. Preview events
        // tunnel from the root, so the menu's chords are matched here before the panel sees them.
        Root.PreviewKeyDown += OnMenuChord;
        if (screen is not null)
            Root.Loaded += async (_, _) => await Opened(_document.Open(screen));
    }

    private async void OnOpen(object sender, RoutedEventArgs e) => await Opened(await _document.OpenAsync());

    private async void OnSave(object sender, RoutedEventArgs e) => await Saved(await _document.SaveAsync());

    private async void OnSaveAs(object sender, RoutedEventArgs e) => await Saved(await _document.SaveAsAsync());

    private void OnMenuChord(object sender, KeyRoutedEventArgs e)
    {
        var modifiers = Modifiers();
        if (modifiers == VirtualKeyModifiers.None) return;
        var item = Menus.Items.SelectMany(menu => menu.Items).OfType<MenuFlyoutItem>().FirstOrDefault(
            item => item.KeyboardAccelerators.Any(a => a.Key == e.Key && a.Modifiers == modifiers));
        if (item is null) return;
        e.Handled = true;
        new MenuFlyoutItemAutomationPeer(item).Invoke();
    }

    private static VirtualKeyModifiers Modifiers()
    {
        static bool Down(VirtualKey key) =>
            InputKeyboardSource.GetKeyStateForCurrentThread(key).HasFlag(CoreVirtualKeyStates.Down);
        return (Down(VirtualKey.Control) ? VirtualKeyModifiers.Control : VirtualKeyModifiers.None)
            | (Down(VirtualKey.Shift) ? VirtualKeyModifiers.Shift : VirtualKeyModifiers.None)
            | (Down(VirtualKey.Menu) ? VirtualKeyModifiers.Menu : VirtualKeyModifiers.None);
    }

    private async Task Opened(bool opened)
    {
        if (opened) ShowSurface();
        else await ShowError();
    }

    private async Task Saved(bool saved)
    {
        if (!saved) await ShowError();
    }

    /// <summary>A new panel per document; the old one frees its host when it unloads.</summary>
    private void ShowSurface()
    {
        var panel = new SlintPanel
        {
            // CPU surfaces only in M1: the swap chain pointer is not passed on, which the panel accepts.
            HostFactory = _ =>
            {
                try
                {
                    return SlintHost.Adopt(_document.NewSurface(1, 1, (float)(Root.XamlRoot?.RasterizationScale ?? 1.0)));
                }
                catch (EditorException ex)
                {
                    throw new SlintException(ex.Message);
                }
            },
        };
        // The panel makes its host in its own Loaded handler, which runs before this one.
        panel.Loaded += async (_, _) =>
        {
            if (panel.Host is null) await ShowError(panel.LastError);
        };
        Surface.Child = panel;
    }

    private async Task ShowError(string? message = null)
    {
        message ??= _document.Error;
        if (message is null || _dialogOpen || Content.XamlRoot is null) return;
        _dialogOpen = true;
        try
        {
            var dialog = new ContentDialog
            {
                Title = "WeftStudio",
                Content = new ScrollViewer { Content = new TextBlock { Text = message, TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true } },
                CloseButtonText = "OK",
                XamlRoot = Content.XamlRoot,
            };
            await dialog.ShowAsync();
        }
        finally
        {
            _dialogOpen = false;
        }
    }
}
