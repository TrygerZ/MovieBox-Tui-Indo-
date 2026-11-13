use crate::providers::dramachi::models::*;

#[test]
fn test_dramachi_search_deserialization_handles_null_data() {
    let raw_json = r#"{
        "data": null,
        "last_page": 1,
        "next_page": 2
    }"#;
    let resp: DramachiSearchResponse =
        serde_json::from_str(raw_json).expect("deserialize empty search");
    assert!(resp.data.is_none());
}

#[test]
fn test_dramachi_search_deserialization_with_items() {
    let raw_json = r#"{
        "data": [
            {
                "id": "61779",
                "title": "Queen of Tears",
                "thumb": "queenoftearsktv.jpg",
                "year": "2024",
                "content": "drama"
            },
            {
                "id": "40770",
                "title": "Parasite 2019",
                "thumb": "parasite2019h.jpg",
                "year": "2019",
                "content": "movies"
            }
        ],
        "last_page": 1,
        "next_page": 2
    }"#;
    let resp: DramachiSearchResponse = serde_json::from_str(raw_json).expect("deserialize items");
    let items = resp.data.expect("items present");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].id, "61779");
    assert_eq!(items[1].content.as_deref(), Some("movies"));
}

#[test]
fn test_dramachi_title_details_deserialization() {
    let raw_json = r#"{
        "album": [
            {
                "id": "61779",
                "title": "Queen of Tears",
                "content": "drama",
                "year": "2024",
                "storyline": "A miraculous love story.",
                "genres": "Drama,Family,Romance"
            }
        ],
        "cast": [
            { "name": "Kim Soo-hyun", "role": null },
            { "name": "Kim Ji-won", "role": null }
        ],
        "seasons": {
            "Season 01": {
                "season_name": "Season 01",
                "versions": [
                    { "version_name": "Original", "rip": "Season 01", "server": "zone1" },
                    { "version_name": "Hindi Dub", "rip": "Season 01 HI DUB", "server": "zone1" }
                ]
            }
        }
    }"#;
    let resp: DramachiTitleDetailsResponse =
        serde_json::from_str(raw_json).expect("deserialize details");
    assert!(resp.album.is_some());
    assert_eq!(resp.album.unwrap()[0].title, "Queen of Tears");
    assert_eq!(resp.cast.unwrap().len(), 2);
    let seasons = resp.seasons.expect("seasons present");
    assert!(seasons.contains_key("Season 01"));
}

#[test]
fn test_dramachi_get_file_deserialization() {
    let raw_json = r#"{
        "fileInfo": [
            {
                "title": "Queen of Tears",
                "f_title": "Queen of Tears S01E16",
                "fid": "628750",
                "disk": "4",
                "url": "zone1/K_Drama/Queen_of_Tears/Season_01/ep16.mkv",
                "filename": "ep16.mkv",
                "size": "218.32 MB"
            }
        ],
        "hostInfo": {
            "host": "o4nbr2fc.pdkvzojivp.xyz",
            "isDL": true
        }
    }"#;
    let resp: DramachiFileInfoResponse =
        serde_json::from_str(raw_json).expect("deserialize file info");
    let file = resp
        .file_info
        .expect("file_info")
        .pop()
        .expect("first file");
    assert_eq!(file.fid.as_deref(), Some("628750"));
    assert_eq!(
        resp.host_info.expect("host_info").host,
        "o4nbr2fc.pdkvzojivp.xyz"
    );
}
