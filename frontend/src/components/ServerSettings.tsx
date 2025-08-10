"use client"

import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import * as z from 'zod';
import { FormInputItem } from "./molecules/form-components/form-input-item";
import { Form } from "./ui/form";
import { Button } from "./ui/button";
import { load } from '@tauri-apps/plugin-store';
import { useEffect } from 'react';
import { useSidebar } from "./Sidebar/SidebarProvider";


const serverSettingsSchema = z.object({
    appServerUrl: z.string().min(1, { message: "App server URL is required" }),
    transcriptServerUrl: z.string().min(1, { message: "Transcript server URL is required" }),
});

type ServerSettings = z.infer<typeof serverSettingsSchema>;

export function ServerSettings(
    {setSaveSuccess}: {setSaveSuccess: (success: boolean) => void}
) {
    const { serverAddress, setServerAddress, transcriptServerAddress, setTranscriptServerAddress } = useSidebar();
    const form = useForm<ServerSettings>({
        resolver: zodResolver(serverSettingsSchema),
        defaultValues: {
            appServerUrl: '',
            transcriptServerUrl: '',
        },
    });
    useEffect(() => {
        const loadSettings = async () => {
            // const store = await load('store.json', { autoSave: false });
            // const appServerUrl = await store.get('appServerUrl') as string | null;
            // const transcriptServerUrl = await store.get('transcriptServerUrl') as string | null;
            
                form.setValue('appServerUrl', serverAddress);
    
            
                form.setValue('transcriptServerUrl', transcriptServerAddress);
            
        };
        loadSettings();
    }, []);
    const onSubmit = async (data: ServerSettings) => {
        try {
        const store = await load('store.json', { autoSave: false });
        await store.set('appServerUrl', data.appServerUrl);
        await store.set('transcriptServerUrl', data.transcriptServerUrl);
        await store.save();
        setSaveSuccess(true);
        setServerAddress(data.appServerUrl);
        setTranscriptServerAddress(data.transcriptServerUrl);
        console.log(data);
        } catch (error) {
            setSaveSuccess(false);
        }
    };
    return (
        <Form {...form}>
            <h3 className="text-lg font-semibold text-gray-900">Server Settings</h3>
            <form onSubmit={form.handleSubmit(onSubmit)} >
                
                <div className="space-y-8 mt-4">
                <FormInputItem 
                    name="appServerUrl"
                    control={form.control}
                    label="App Server URL"
                    type="text"
                    placeholder="Enter app server URL"
                />
                <FormInputItem 
                    name="transcriptServerUrl"
                    control={form.control}
                    label="Transcript Server URL"
                    type="text"
                    placeholder="Enter transcript server URL"
                />
                </div>
                <div className="flex justify-end mt-6">
                <Button type="submit" className="bg-blue-600 hover:bg-blue-700 text-white px-4 py-2 rounded-md text-sm font-medium">Save</Button>
                </div>
            </form>
        </Form>
    );
}
