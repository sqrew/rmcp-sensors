use rmcp::{
    handler::server::{router::tool::ToolRouter, ServerHandler, wrapper::Parameters},
    model::*,
    ErrorData as McpError,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct WeatherServer {
    pub tool_router: ToolRouter<Self>,
    client: reqwest::Client,
}

impl Default for WeatherServer {
    fn default() -> Self {
        Self::new()
    }
}

impl WeatherServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            client: reqwest::Client::new(),
        }
    }

    async fn fetch_weather(&self, location: &str) -> Result<WttrResponse, McpError> {
        let url = format!("https://wttr.in/{}?format=j1", urlencoding::encode(location));

        let response = self.client
            .get(&url)
            .header("User-Agent", "rmcp-weather/0.1.0")
            .send()
            .await
            .map_err(|e| McpError::internal_error(format!("HTTP request failed: {}", e), None))?;

        if !response.status().is_success() {
            return Err(McpError::internal_error(
                format!("Weather API returned status: {}", response.status()),
                None,
            ));
        }

        response
            .json::<WttrResponse>()
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to parse weather data: {}", e), None))
    }
}

// wttr.in JSON response structures
#[derive(Debug, Deserialize)]
pub struct WttrResponse {
    pub current_condition: Vec<CurrentCondition>,
    pub nearest_area: Vec<NearestArea>,
    pub weather: Vec<WeatherDay>,
}

#[derive(Debug, Deserialize)]
pub struct CurrentCondition {
    pub temp_F: String,
    pub temp_C: String,
    #[serde(rename = "FeelsLikeF")]
    pub feels_like_f: String,
    #[serde(rename = "FeelsLikeC")]
    pub feels_like_c: String,
    pub humidity: String,
    pub weatherDesc: Vec<WeatherDesc>,
    pub windspeedMiles: String,
    pub windspeedKmph: String,
    pub winddir16Point: String,
    pub precipMM: String,
    pub visibility: String,
    pub pressure: String,
    pub uvIndex: String,
}

#[derive(Debug, Deserialize)]
pub struct WeatherDesc {
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct NearestArea {
    pub areaName: Vec<AreaValue>,
    pub region: Vec<AreaValue>,
    pub country: Vec<AreaValue>,
}

#[derive(Debug, Deserialize)]
pub struct AreaValue {
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct WeatherDay {
    pub date: String,
    pub maxtempF: String,
    pub maxtempC: String,
    pub mintempF: String,
    pub mintempC: String,
    pub hourly: Vec<HourlyForecast>,
}

#[derive(Debug, Deserialize)]
pub struct HourlyForecast {
    pub time: String,
    pub tempF: String,
    pub tempC: String,
    pub weatherDesc: Vec<WeatherDesc>,
    pub chanceofrain: String,
}

// Tool parameter structs
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LocationParams {
    #[schemars(description = "Location to get weather for (city name, zip code, or 'lat,lon')")]
    pub location: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ForecastParams {
    #[schemars(description = "Location to get forecast for")]
    pub location: String,
    #[schemars(description = "Number of days (1-3, default 3)")]
    #[serde(default)]
    pub days: Option<u8>,
}

#[rmcp::tool_router]
impl WeatherServer {
    #[rmcp::tool(description = "Get current weather conditions for a location")]
    pub async fn get_weather(
        &self,
        Parameters(params): Parameters<LocationParams>,
    ) -> Result<CallToolResult, McpError> {
        let data = self.fetch_weather(&params.location).await?;

        let current = data.current_condition.first()
            .ok_or_else(|| McpError::internal_error("No current conditions", None))?;

        let area = data.nearest_area.first()
            .map(|a| format!("{}, {}",
                a.areaName.first().map(|v| v.value.as_str()).unwrap_or("Unknown"),
                a.region.first().map(|v| v.value.as_str()).unwrap_or("")
            ))
            .unwrap_or_else(|| params.location.clone());

        let desc = current.weatherDesc.first()
            .map(|d| d.value.as_str())
            .unwrap_or("Unknown");

        let output = format!(
            "Weather for {}:\n\
             Conditions: {}\n\
             Temperature: {}°F / {}°C\n\
             Feels like: {}°F / {}°C\n\
             Humidity: {}%\n\
             Wind: {} mph {} ({})\n\
             Visibility: {} miles\n\
             Pressure: {} mb\n\
             UV Index: {}",
            area, desc,
            current.temp_F, current.temp_C,
            current.feels_like_f, current.feels_like_c,
            current.humidity,
            current.windspeedMiles, current.winddir16Point, current.windspeedKmph,
            current.visibility,
            current.pressure,
            current.uvIndex
        );

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[rmcp::tool(description = "Get weather forecast for upcoming days")]
    pub async fn get_forecast(
        &self,
        Parameters(params): Parameters<ForecastParams>,
    ) -> Result<CallToolResult, McpError> {
        let data = self.fetch_weather(&params.location).await?;
        let days = params.days.unwrap_or(3).min(3) as usize;

        let area = data.nearest_area.first()
            .map(|a| format!("{}, {}",
                a.areaName.first().map(|v| v.value.as_str()).unwrap_or("Unknown"),
                a.region.first().map(|v| v.value.as_str()).unwrap_or("")
            ))
            .unwrap_or_else(|| params.location.clone());

        let mut output = format!("Forecast for {} ({} days):\n\n", area, days);

        for day in data.weather.iter().take(days) {
            output.push_str(&format!(
                "{}:\n  High: {}°F / {}°C | Low: {}°F / {}°C\n",
                day.date, day.maxtempF, day.maxtempC, day.mintempF, day.mintempC
            ));

            // Show a few hourly forecasts
            for hour in day.hourly.iter().step_by(3) {
                let time_hr = hour.time.parse::<u32>().unwrap_or(0) / 100;
                let desc = hour.weatherDesc.first()
                    .map(|d| d.value.as_str())
                    .unwrap_or("?");
                output.push_str(&format!(
                    "  {:02}:00 - {}°F, {}, {}% rain\n",
                    time_hr, hour.tempF, desc, hour.chanceofrain
                ));
            }
            output.push('\n');
        }

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }
}

#[rmcp::tool_handler]
impl ServerHandler for WeatherServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("Weather information server using wttr.in".into()),
        }
    }
}
