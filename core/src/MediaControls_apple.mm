// MediaControls on macOS and iOS: MPNowPlayingInfoCenter publishes what is
// playing, MPRemoteCommandCenter sends the commands back. Compiled with ARC
// (core/CMakeLists.txt).

#include "MediaControls.h"

#include <vector>

#import <MediaPlayer/MediaPlayer.h>

#if TARGET_OS_OSX
#import <AppKit/AppKit.h>
#else
#import <UIKit/UIKit.h>
#endif

/** What the command handlers reach the owner through. The handlers hold it
    weakly, and the backend clears `owner` when it detaches, so a handler
    that runs late does nothing. */
@interface AnompMediaCommandTarget : NSObject
@property (nonatomic, assign) anomp::MediaControls* owner;
@end

@implementation AnompMediaCommandTarget
@end

namespace anomp
{
namespace
{
#if TARGET_OS_OSX
using PlatformImage = NSImage;
#else
using PlatformImage = UIImage;
#endif

PlatformImage* makeImage (const juce::MemoryBlock& data)
{
    if (data.isEmpty())
        return nil;

    NSData* bytes = [NSData dataWithBytes:data.getData() length:data.getSize()];
#if TARGET_OS_OSX
    PlatformImage* image = [[NSImage alloc] initWithData:bytes];
#else
    PlatformImage* image = [UIImage imageWithData:bytes];
#endif
    return image != nil && image.size.width > 0 && image.size.height > 0 ? image : nil;
}

NSString* toNSString (const juce::String& text) { return [NSString stringWithUTF8String:text.toRawUTF8()]; }

MPRemoteCommandHandlerStatus deliver (AnompMediaCommandTarget* target, MediaControls::Command command, double position)
{
    auto* owner = target.owner;

    if (owner == nullptr || ! owner->getState().hasTrack)
        return MPRemoteCommandHandlerStatusNoActionableNowPlayingItem;

    return owner->handleCommand (command, position) ? MPRemoteCommandHandlerStatusSuccess
                                                    : MPRemoteCommandHandlerStatusCommandFailed;
}

class AppleBackend final : public MediaControls::Backend
{
public:
    ~AppleBackend() override { detach(); }

    void attach (MediaControls& owner) override
    {
        @autoreleasepool
        {
            target = [AnompMediaCommandTarget new];
            target.owner = &owner;

            auto* center = [MPRemoteCommandCenter sharedCommandCenter];
            add (center.playCommand, MediaControls::Command::play);
            add (center.pauseCommand, MediaControls::Command::pause);
            add (center.togglePlayPauseCommand, MediaControls::Command::toggle);
            add (center.nextTrackCommand, MediaControls::Command::next);
            add (center.previousTrackCommand, MediaControls::Command::previous);
            add (center.changePlaybackPositionCommand, MediaControls::Command::seek);
            center.nextTrackCommand.enabled = NO;
            center.previousTrackCommand.enabled = NO;
        }
    }

    void detach() override
    {
        if (target == nil)
            return;

        @autoreleasepool
        {
            target.owner = nullptr;
            target = nil;

            for (const auto& registration : registrations)
                [registration.command removeTarget:registration.token];
            registrations.clear();

            artwork = nil;
            clearInfo();
        }
    }

    void update (const MediaControls::State& state, int changes) override
    {
        @autoreleasepool
        {
            if (! state.hasTrack)
            {
                artwork = nil;
                clearInfo();
                return;
            }

            if ((changes & MediaControls::artworkChanged) != 0)
            {
                auto* image = makeImage (state.artwork);
                artwork = image == nil ? nil
                                       : [[MPMediaItemArtwork alloc] initWithBoundsSize:image.size
                                                                         requestHandler:^PlatformImage*(CGSize) {
                                                                           return image;
                                                                         }];
            }

            if ((changes & MediaControls::navigationChanged) != 0)
            {
                auto* center = [MPRemoteCommandCenter sharedCommandCenter];
                center.nextTrackCommand.enabled = state.hasNext ? YES : NO;
                center.previousTrackCommand.enabled = state.hasPrevious ? YES : NO;
            }

            const auto playing = state.playback == MediaControls::Playback::playing;
            NSMutableDictionary* info = [NSMutableDictionary dictionary];
            info[MPNowPlayingInfoPropertyMediaType] = @(MPNowPlayingInfoMediaTypeAudio);

            if (state.title.isNotEmpty())
                info[MPMediaItemPropertyTitle] = toNSString (state.title);
            if (state.artist.isNotEmpty())
                info[MPMediaItemPropertyArtist] = toNSString (state.artist);
            if (state.album.isNotEmpty())
                info[MPMediaItemPropertyAlbumTitle] = toNSString (state.album);
            if (state.duration > 0.0)
                info[MPMediaItemPropertyPlaybackDuration] = @(state.duration);
            if (artwork != nil)
                info[MPMediaItemPropertyArtwork] = artwork;

            info[MPNowPlayingInfoPropertyElapsedPlaybackTime] = @(state.elapsed);
            info[MPNowPlayingInfoPropertyPlaybackRate] = @(playing ? 1.0 : 0.0);
            info[MPNowPlayingInfoPropertyDefaultPlaybackRate] = @1.0;

            auto* center = [MPNowPlayingInfoCenter defaultCenter];
            center.nowPlayingInfo = info;
            // macOS decides which app is "now playing" (and gets the media
            // keys) from this; iOS takes it from the audio session instead.
            center.playbackState = playing ? MPNowPlayingPlaybackStatePlaying : MPNowPlayingPlaybackStatePaused;
        }
    }

    bool canShowArtwork (const juce::MemoryBlock& data) override
    {
        @autoreleasepool
        {
            return makeImage (data) != nil;
        }
    }

private:
    struct Registration
    {
        MPRemoteCommand* command;
        id token;
    };

    void add (MPRemoteCommand* command, MediaControls::Command kind)
    {
        __weak AnompMediaCommandTarget* weakTarget = target;
        id token = [command addTargetWithHandler:^MPRemoteCommandHandlerStatus (MPRemoteCommandEvent* event) {
          double position = 0.0;
          if ([event isKindOfClass:[MPChangePlaybackPositionCommandEvent class]])
              position = ((MPChangePlaybackPositionCommandEvent*) event).positionTime;

          if ([NSThread isMainThread])
              return deliver (weakTarget, kind, position);

          // The owner is main-thread only. Handlers arrive there in
          // practice; if one doesn't, pass it on without waiting.
          dispatch_async (dispatch_get_main_queue(), ^{
            deliver (weakTarget, kind, position);
          });
          return MPRemoteCommandHandlerStatusSuccess;
        }];
        command.enabled = YES;
        registrations.push_back ({ command, token });
    }

    static void clearInfo()
    {
        auto* center = [MPNowPlayingInfoCenter defaultCenter];
        center.nowPlayingInfo = nil;
        center.playbackState = MPNowPlayingPlaybackStateStopped;
    }

    AnompMediaCommandTarget* target = nil;
    std::vector<Registration> registrations;
    MPMediaItemArtwork* artwork = nil;
};
} // namespace

std::unique_ptr<MediaControls::Backend> MediaControls::createPlatformBackend()
{
    return std::make_unique<AppleBackend>();
}

bool MediaControls::isSupported() { return true; }
} // namespace anomp
