import { invoke } from "@tauri-apps/api/core";

export type Session = { site: string; token: string };
export type Info = { userid: number; fullname: string; sitename: string };
export type Filter = "all" | "inprogress" | "future" | "past" | "favourites" | "hidden";
export type Course = { id: number; fullname: string; shortname: string; progress?: number | null; courseimage?: string; isfavourite?: boolean; hidden?: boolean };
export type FileItem = { type: string; filename: string; fileurl: string; filesize: number };
export type Module = { id: number; name: string; modname: string; url?: string; contents?: FileItem[] };
export type Section = { id: number; name: string; modules: Module[] };

export const resolveSite = (input: string) => invoke<{ site: string; config: any }>("resolve_site", { input });
export const startSso = (site: string) => invoke<void>("start_sso", { site });
export const loginPassword = (site: string, username: string, password: string) =>
invoke<void>("login_password", { site, username, password });
export const getSession = () => invoke<Session | null>("get_session");
export const logout = () => invoke<void>("logout");

const ws = <T,>(fn: string, params: Record<string, string> = {}) => invoke<T>("ws_call", { function: fn, params });
export const siteInfo = () => ws<Info>("core_webservice_get_site_info");
export const listCourses = async (classification: Filter) =>
(await ws<{ courses: Course[] }>("core_course_get_enrolled_courses_by_timeline_classification",
                                 { classification, limit: "0", offset: "0", sort: "fullname" })).courses;
                                 export const setFavourite = (id: number, fav: boolean) =>
                                 ws("core_course_set_favourite_courses", { "courses[0][id]": String(id), "courses[0][favourite]": fav ? "1" : "0" });
                                 // Moodle treats a course as hidden when block_myoverview_hidden_course_<id> EXISTS (any value),
                                 // so un-hiding must delete the preference. update_user_preferences deletes it when "value" is omitted.
                                 export const setHidden = (uid: number, id: number, hide: boolean) => {
                                     const name = `block_myoverview_hidden_course_${id}`;
                                     return hide
                                     ? ws("core_user_set_user_preferences", { "preferences[0][name]": name, "preferences[0][value]": "1", "preferences[0][userid]": String(uid) })
                                     : ws("core_user_update_user_preferences", { userid: String(uid), "preferences[0][type]": name });
                                 };
                                 export const courseContents = (id: number) => ws<Section[]>("core_course_get_contents", { courseid: String(id) });
                                 export const download = (url: string, dir: string, subdir: string, filename: string) =>
                                 invoke<string>("download_file", { url, dir, subdir, filename });
