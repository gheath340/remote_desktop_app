#import <Foundation/Foundation.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#import <CoreVideo/CoreVideo.h>
#import "maccapture.h"

@interface SCKBridge : NSObject<SCStreamOutput, SCStreamDelegate>
@property(nonatomic, assign) sck_frame_cb cb;
@end

@implementation SCKBridge

- (void)stream:(SCStream *)stream
didOutputSampleBuffer:(CMSampleBufferRef)sampleBuffer
        ofType:(SCStreamOutputType)outputType
{
    if (outputType != SCStreamOutputTypeScreen) return;

    CVImageBufferRef imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer);
    if (!imageBuffer) return;

    CVPixelBufferLockBaseAddress(imageBuffer, kCVPixelBufferLock_ReadOnly);

    size_t width        = CVPixelBufferGetWidth(imageBuffer);
    size_t height       = CVPixelBufferGetHeight(imageBuffer);
    size_t bytesPerRow  = CVPixelBufferGetBytesPerRow(imageBuffer);
    void *baseAddress   = CVPixelBufferGetBaseAddress(imageBuffer);

    if (self.cb && baseAddress) {
        //NSLog(@"Frame captured %zux%zu", width, height);
        self.cb((const uint8_t*)baseAddress,
                (uint32_t)width,
                (uint32_t)height,
                (uint32_t)bytesPerRow);
    }

    CVPixelBufferUnlockBaseAddress(imageBuffer, kCVPixelBufferLock_ReadOnly);
}

@end

// Global references (to keep the stream alive)
static SCStream *globalStream = nil;
static SCKBridge *globalBridge = nil;

void sck_start_capture(sck_frame_cb cb) {
    @autoreleasepool {
        // Run ScreenCaptureKit in a dedicated background thread
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INTERACTIVE, 0), ^{
            @autoreleasepool {
                NSLog(@"[SCK] Initializing capture thread…");

                [SCShareableContent getShareableContentWithCompletionHandler:^(SCShareableContent *content, NSError *error) {

                    if (error || !content) {
                        NSLog(@"ScreenCaptureKit content error: %@", error);
                        return;
                    }

                    NSLog(@"Discovered %lu displays, %lu windows",
                        (unsigned long)content.displays.count,
                        (unsigned long)content.windows.count);

                    SCDisplay *display = content.displays.firstObject;
                    if (!display) {
                        NSLog(@"No display found");
                        return;
                    }

                    // Configure display capture
                    SCContentFilter *filter = [[SCContentFilter alloc] initWithDisplay:display excludingWindows:@[]];
                    SCStreamConfiguration *config = [SCStreamConfiguration new];
                    config.pixelFormat = kCVPixelFormatType_32BGRA;
                    config.showsCursor = YES;
                    config.width  = display.width;
                    config.height = display.height;
                    config.minimumFrameInterval = CMTimeMake(1, 60);
                    config.scalesToFit = YES;

                    // Initialize global stream & bridge
                    globalBridge = [SCKBridge new];
                    globalBridge.cb = cb;

                    NSError *addErr = nil;
                    globalStream = [[SCStream alloc] initWithFilter:filter
                                                     configuration:config
                                                         delegate:globalBridge];

                    BOOL success = [globalStream addStreamOutput:globalBridge
                                                           type:SCStreamOutputTypeScreen
                                            sampleHandlerQueue:dispatch_get_global_queue(QOS_CLASS_USER_INTERACTIVE, 0)
                                                         error:&addErr];

                    if (!success || addErr) {
                        NSLog(@"addStreamOutput error: %@", addErr);
                        return;
                    }

                    // Delay slightly before starting
                    // Start the capture on this same background thread — not the main queue
                    dispatch_after(
                        dispatch_time(DISPATCH_TIME_NOW, (int64_t)(0.2 * NSEC_PER_SEC)),
                        dispatch_get_global_queue(QOS_CLASS_USER_INTERACTIVE, 0),
                        ^{
                            [globalStream startCaptureWithCompletionHandler:^(NSError * _Nullable startErr) {
                                if (startErr) {
                                    NSLog(@"startCapture error: %@", startErr);
                                } else {
                                    NSLog(@"ScreenCaptureKit stream started (macOS 15+)");
                                }
                            }];
                        }
                    );


                    // 🔥 Keep run loop alive so ScreenCaptureKit can deliver frames
                    NSLog(@"[SCK] Entering CFRunLoop…");
                    CFRunLoopRun();
                }];
            }
        });
    }
}
